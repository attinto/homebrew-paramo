use crate::blocker::{self, SyncAction};
use crate::config::SystemConfig;
use crate::hosts;
use crate::i18n::{I18n, Language};
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
    lines.push(match i18n.language() {
        Language::Es => format!("Directorio de soporte listo: {}", paths::APP_SUPPORT_DIR),
        Language::En => format!("Support directory ready: {}", paths::APP_SUPPORT_DIR),
    });

    lines.push(config_install_message(
        migrate_or_create_config(config)?,
        i18n,
    ));
    lines.extend(cleanup_legacy_installation(i18n)?);

    let active_config = SystemConfig::load_from(&paths::system_config_file())
        .context("failed to reload active configuration after install preparation")?;
    let sync_action = blocker::run(&active_config)?;
    lines.push(sync_action_message(sync_action, i18n));

    write_plist(&binary_path, active_config.daemon.interval_seconds)?;
    lines.push(match i18n.language() {
        Language::Es => format!("LaunchDaemon actualizado: {}", paths::PLIST_DEST),
        Language::En => format!("LaunchDaemon updated: {}", paths::PLIST_DEST),
    });

    bootstrap_launchd()?;
    let status = launchd::query_service(paths::LAUNCHD_LABEL)?;
    if !status.loaded {
        anyhow::bail!("launchd did not load {}", paths::LAUNCHD_LABEL);
    }
    lines.push(launchd_status_message(&status, i18n));

    lines.push(if binary_path == Path::new(paths::MANUAL_BINARY_DEST) {
        match i18n.language() {
            Language::Es => format!("Binario copiado a {}", paths::MANUAL_BINARY_DEST),
            Language::En => format!("Binary copied to {}", paths::MANUAL_BINARY_DEST),
        }
    } else {
        match i18n.language() {
            Language::Es => format!(
                "Se usará el binario ya instalado en {}",
                binary_path.display()
            ),
            Language::En => format!(
                "Using the existing installed binary at {}",
                binary_path.display()
            ),
        }
    });

    lines.push(i18n.install_completed().to_string());
    Ok(InstallSummary { lines })
}

pub fn uninstall(i18n: I18n) -> Result<InstallSummary> {
    let mut lines = vec![i18n.uninstall_started().to_string()];

    if remove_managed_hosts_block()? {
        lines.push(match i18n.language() {
            Language::Es => "Bloque de PARAMO retirado de /etc/hosts.".to_string(),
            Language::En => "Removed the PARAMO block from /etc/hosts.".to_string(),
        });
    }

    let unloaded = launchd::bootout_service(paths::LAUNCHD_LABEL)?;
    let plist_path = Path::new(paths::PLIST_DEST);
    if plist_path.exists() {
        fs::remove_file(plist_path).context("failed to remove plist")?;
        lines.push(match i18n.language() {
            Language::Es => "LaunchDaemon eliminado.".to_string(),
            Language::En => "LaunchDaemon removed.".to_string(),
        });
    } else if unloaded {
        lines.push(match i18n.language() {
            Language::Es => "Servicio de launchd descargado.".to_string(),
            Language::En => "launchd service unloaded.".to_string(),
        });
    }

    let manual_binary = Path::new(paths::MANUAL_BINARY_DEST);
    if manual_binary.exists() && !manual_binary.is_symlink() {
        fs::remove_file(manual_binary).context("failed to remove manual binary")?;
        lines.push(match i18n.language() {
            Language::Es => format!("Binario manual eliminado: {}", paths::MANUAL_BINARY_DEST),
            Language::En => format!("Manual binary removed: {}", paths::MANUAL_BINARY_DEST),
        });
    }

    lines.push(match i18n.language() {
        Language::Es => "La configuración en /etc/paramo se conserva.".to_string(),
        Language::En => "Configuration in /etc/paramo was preserved.".to_string(),
    });
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
    match (action, i18n.language()) {
        (ConfigInstallAction::Preserved, Language::Es) => format!(
            "Configuración activa validada y normalizada: {}",
            paths::SYSTEM_CONFIG_FILE
        ),
        (ConfigInstallAction::Preserved, Language::En) => format!(
            "Validated and normalized the active configuration: {}",
            paths::SYSTEM_CONFIG_FILE
        ),
        (ConfigInstallAction::Migrated, Language::Es) => format!(
            "Configuración migrada desde {} a {}",
            paths::LEGACY_SYSTEM_CONFIG_FILE,
            paths::SYSTEM_CONFIG_FILE
        ),
        (ConfigInstallAction::Migrated, Language::En) => format!(
            "Migrated the configuration from {} to {}",
            paths::LEGACY_SYSTEM_CONFIG_FILE,
            paths::SYSTEM_CONFIG_FILE
        ),
        (ConfigInstallAction::Created, Language::Es) => {
            format!("Configuración creada en {}", paths::SYSTEM_CONFIG_FILE)
        }
        (ConfigInstallAction::Created, Language::En) => {
            format!("Configuration created at {}", paths::SYSTEM_CONFIG_FILE)
        }
    }
}

fn sync_action_message(action: SyncAction, i18n: I18n) -> String {
    match (action, i18n.language()) {
        (SyncAction::Blocked, Language::Es) => {
            "Bloqueo inicial sincronizado con /etc/hosts.".to_string()
        }
        (SyncAction::Blocked, Language::En) => {
            "Initial blocking state synchronized with /etc/hosts.".to_string()
        }
        (SyncAction::Unblocked, Language::Es) => {
            "Se limpió el bloque gestionado en /etc/hosts.".to_string()
        }
        (SyncAction::Unblocked, Language::En) => {
            "Cleared the managed block from /etc/hosts.".to_string()
        }
        (SyncAction::Updated, Language::Es) => {
            "Bloque de hosts actualizado para reflejar la configuración actual.".to_string()
        }
        (SyncAction::Updated, Language::En) => {
            "Updated the hosts block to match the current configuration.".to_string()
        }
        (SyncAction::AlreadyCorrect(_), Language::Es) => {
            "El estado de /etc/hosts ya coincidía con la configuración.".to_string()
        }
        (SyncAction::AlreadyCorrect(_), Language::En) => {
            "The /etc/hosts state already matched the configuration.".to_string()
        }
    }
}

fn write_plist(binary_path: &Path, interval_seconds: u32) -> Result<()> {
    let rendered = launchd::render_plist(binary_path, interval_seconds);
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

fn bootstrap_launchd() -> Result<()> {
    let _ = launchd::bootout_service(paths::LAUNCHD_LABEL)?;
    launchd::bootstrap_target(paths::PLIST_DEST)?;
    launchd::kickstart_service(paths::LAUNCHD_LABEL)?;
    Ok(())
}

fn cleanup_legacy_installation(i18n: I18n) -> Result<Vec<String>> {
    let mut lines = Vec::new();

    if launchd::bootout_service(paths::LEGACY_LAUNCHD_LABEL)? {
        lines.push(match i18n.language() {
            Language::Es => "Servicio legacy de Undistracted descargado.".to_string(),
            Language::En => "Unloaded the legacy Undistracted service.".to_string(),
        });
    }

    let legacy_plist = Path::new(paths::LEGACY_PLIST_DEST);
    if legacy_plist.exists() {
        fs::remove_file(legacy_plist).context("failed to remove legacy plist")?;
        lines.push(match i18n.language() {
            Language::Es => format!(
                "LaunchDaemon legacy eliminado: {}",
                paths::LEGACY_PLIST_DEST
            ),
            Language::En => format!(
                "Removed the legacy LaunchDaemon: {}",
                paths::LEGACY_PLIST_DEST
            ),
        });
    }

    Ok(lines)
}

fn launchd_status_message(status: &launchd::ServiceStatus, i18n: I18n) -> String {
    let pid_suffix = status
        .pid
        .as_deref()
        .map(|pid| match i18n.language() {
            Language::Es => format!(" (pid {pid})"),
            Language::En => format!(" (pid {pid})"),
        })
        .unwrap_or_default();

    match i18n.language() {
        Language::Es => format!(
            "Daemon verificado en launchd: {}{}",
            paths::LAUNCHD_LABEL,
            pid_suffix
        ),
        Language::En => format!(
            "Verified the daemon in launchd: {}{}",
            paths::LAUNCHD_LABEL,
            pid_suffix
        ),
    }
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
