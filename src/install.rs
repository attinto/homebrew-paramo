use crate::blocker::{self, SyncAction};
use crate::config::SystemConfig;
use crate::hosts;
use crate::i18n::I18n;
use crate::launchd;
use crate::paths;
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

#[derive(Debug, Clone)]
pub struct InstallSummary {
    pub lines: Vec<String>,
}

pub fn install(config: &SystemConfig, i18n: I18n) -> Result<InstallSummary> {
    let mut lines = vec![i18n.install_started().to_string()];

    let binary_path = resolve_binary_path()?;
    fs::create_dir_all(paths::system_config_dir()).context("failed to create support dir")?;
    lines.push(i18n.format("install_support_dir_ready", &[paths::APP_SUPPORT_DIR]));

    lines.push(config_install_message(
        migrate_or_create_config(config)?,
        i18n,
    ));
    set_config_owner_to_sudo_user();
    lines.extend(cleanup_legacy_installation(i18n)?);

    let active_config = SystemConfig::load_from(&paths::system_config_file())
        .context("failed to reload active configuration after install preparation")?;
    let sync_action = blocker::run(&active_config)?;
    lines.push(sync_action_message(sync_action, i18n));

    write_plist(&binary_path)?;
    lines.push(i18n.format("install_launchd_updated", &[paths::PLIST_DEST]));

    bootstrap_launchd()?;
    let status = launchd::query_service(paths::LAUNCHD_LABEL)?;
    if !status.loaded {
        anyhow::bail!("launchd did not load {}", paths::LAUNCHD_LABEL);
    }
    lines.push(launchd_status_message(&status, i18n));

    lines.push(if binary_path == Path::new(paths::MANUAL_BINARY_DEST) {
        i18n.format("install_binary_copied", &[paths::MANUAL_BINARY_DEST])
    } else {
        i18n.format(
            "install_binary_using_existing",
            &[&binary_path.display().to_string()],
        )
    });

    lines.push(i18n.install_completed().to_string());
    Ok(InstallSummary { lines })
}

pub fn uninstall(i18n: I18n) -> Result<InstallSummary> {
    let mut lines = vec![i18n.uninstall_started().to_string()];

    if remove_managed_hosts_block()? {
        lines.push(i18n.t("uninstall_hosts_block_removed").to_string());
    }

    let unloaded = launchd::bootout_service(paths::LAUNCHD_LABEL)?;
    let plist_path = Path::new(paths::PLIST_DEST);
    if plist_path.exists() {
        fs::remove_file(plist_path).context("failed to remove plist")?;
        lines.push(i18n.t("uninstall_launchd_removed").to_string());
    } else if unloaded {
        lines.push(i18n.t("uninstall_launchd_unloaded").to_string());
    }

    let manual_binary = Path::new(paths::MANUAL_BINARY_DEST);
    if manual_binary.exists() && !manual_binary.is_symlink() {
        fs::remove_file(manual_binary).context("failed to remove manual binary")?;
        lines.push(i18n.format("uninstall_binary_removed", &[paths::MANUAL_BINARY_DEST]));
    }

    lines.push(i18n.t("uninstall_config_preserved").to_string());
    lines.push(i18n.uninstall_completed().to_string());

    Ok(InstallSummary { lines })
}

fn resolve_binary_path() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;

    if is_homebrew_binary(&current_exe) || is_global_binary(&current_exe) {
        return Ok(prefer_stable_binary_path(current_exe));
    }

    let target = PathBuf::from(paths::MANUAL_BINARY_DEST);
    fs::copy(&current_exe, &target).context("failed to copy binary")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
        .context("failed to set binary permissions")?;

    Ok(target)
}

fn prefer_stable_binary_path(current_exe: PathBuf) -> PathBuf {
    for candidate in paths::brew_bin_candidates() {
        if candidate.exists() {
            return candidate;
        }
    }

    current_exe
}

fn is_homebrew_binary(path: &Path) -> bool {
    let text = path.display().to_string();
    text.contains("/Cellar/")
        || text.starts_with("/opt/homebrew/bin/")
        || text.starts_with("/usr/local/bin/")
}

fn is_global_binary(path: &Path) -> bool {
    path == Path::new(paths::MANUAL_BINARY_DEST)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigInstallAction {
    Preserved,
    Migrated,
    Created,
}

fn migrate_or_create_config(config: &SystemConfig) -> Result<ConfigInstallAction> {
    let active_config = paths::system_config_file();
    if active_config.exists() {
        let normalized = SystemConfig::load_from(&active_config)?;
        normalized.save_active()?;
        return Ok(ConfigInstallAction::Preserved);
    }

    let legacy_path = Path::new(paths::LEGACY_SYSTEM_CONFIG_FILE);
    if legacy_path.exists() {
        let legacy_config = SystemConfig::load_from(legacy_path)?;
        legacy_config.save_active()?;
        return Ok(ConfigInstallAction::Migrated);
    }

    config.save_active()?;
    Ok(ConfigInstallAction::Created)
}

fn config_install_message(action: ConfigInstallAction, i18n: I18n) -> String {
    match action {
        ConfigInstallAction::Preserved => {
            i18n.format("install_config_preserved", &[paths::SYSTEM_CONFIG_FILE])
        }
        ConfigInstallAction::Migrated => i18n.format(
            "install_config_migrated",
            &[paths::LEGACY_SYSTEM_CONFIG_FILE, paths::SYSTEM_CONFIG_FILE],
        ),
        ConfigInstallAction::Created => {
            i18n.format("install_config_created", &[paths::SYSTEM_CONFIG_FILE])
        }
    }
}

fn sync_action_message(action: SyncAction, i18n: I18n) -> String {
    let key = match action {
        SyncAction::Blocked => "install_sync_blocked",
        SyncAction::Unblocked => "install_sync_unblocked",
        SyncAction::Updated => "install_sync_updated",
        SyncAction::AlreadyCorrect(_) => "install_sync_already_correct",
    };
    i18n.t(key).to_string()
}

fn write_plist(binary_path: &Path) -> Result<()> {
    let rendered = launchd::render_plist(binary_path);
    write_file_atomically(Path::new(paths::PLIST_DEST), rendered.as_bytes(), 0o644)?;
    let status = std::process::Command::new("chown")
        .args(["root:wheel", paths::PLIST_DEST])
        .status()
        .context("failed to set plist owner")?;
    if !status.success() {
        anyhow::bail!("failed to set plist owner for {}", paths::PLIST_DEST);
    }
    Ok(())
}

fn write_file_atomically(path: &Path, content: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .context("cannot write a file atomically without a parent directory")?;
    fs::create_dir_all(parent)?;

    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(content)?;
    temp.as_file_mut().sync_all()?;
    temp.as_file_mut()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn set_config_owner_to_sudo_user() {
    let sudo_user = match std::env::var("SUDO_USER") {
        Ok(user) if !user.is_empty() && user != "root" => user,
        _ => return,
    };
    let _ = std::process::Command::new("chown")
        .args([&sudo_user, paths::APP_SUPPORT_DIR])
        .status();
    let config_file = paths::SYSTEM_CONFIG_FILE;
    if Path::new(config_file).exists() {
        let _ = std::process::Command::new("chown")
            .args([&sudo_user, config_file])
            .status();
    }
}

fn bootstrap_launchd() -> Result<()> {
    let _ = launchd::bootout_service(paths::LAUNCHD_LABEL)?;
    launchd::bootstrap_target(paths::PLIST_DEST)?;
    launchd::kickstart_service(paths::LAUNCHD_LABEL)?;
    Ok(())
}

fn cleanup_legacy_installation(i18n: I18n) -> Result<Vec<String>> {
    let mut lines = Vec::new();

    if launchd::bootout_service(paths::LEGACY_LAUNCHD_LABEL)? {
        lines.push(i18n.t("install_legacy_unloaded").to_string());
    }

    let legacy_plist = Path::new(paths::LEGACY_PLIST_DEST);
    if legacy_plist.exists() {
        fs::remove_file(legacy_plist).context("failed to remove legacy plist")?;
        lines.push(i18n.format("install_legacy_plist_removed", &[paths::LEGACY_PLIST_DEST]));
    }

    Ok(lines)
}

fn launchd_status_message(status: &launchd::ServiceStatus, i18n: I18n) -> String {
    let pid_suffix = status
        .pid
        .as_deref()
        .map(|pid| i18n.format("install_pid_suffix", &[pid]))
        .unwrap_or_default();

    i18n.format(
        "install_launchd_verified",
        &[paths::LAUNCHD_LABEL, &pid_suffix],
    )
}

fn remove_managed_hosts_block() -> Result<bool> {
    let config = SystemConfig::load().unwrap_or_default();
    let hosts_path = &config.hosts.file;
    let content = match hosts::read_hosts(hosts_path) {
        Ok(content) => content,
        Err(_) => return Ok(false),
    };

    if !hosts::is_blocked(&content, &config.hosts.marker) {
        return Ok(false);
    }

    blocker::unblock_now(&config)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn test_sync_action_message_is_stable() {
        let text = sync_action_message(SyncAction::Updated, I18n::new(Language::En));
        assert!(text.contains("hosts"));
    }

    #[test]
    fn test_config_install_message_mentions_destination() {
        let text = config_install_message(ConfigInstallAction::Migrated, I18n::new(Language::Es));
        assert!(text.contains(paths::SYSTEM_CONFIG_FILE));
    }
}
