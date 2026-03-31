use crate::config::SystemConfig;
use crate::i18n::{I18n, Language};
use crate::paths;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct InstallSummary {
    pub lines: Vec<String>,
}

pub fn install(config: &SystemConfig, i18n: I18n) -> Result<InstallSummary> {
    let mut lines = vec![i18n.install_started().to_string()];

    let binary_path = resolve_binary_path()?;
    let plist_binary_path = binary_path.display().to_string();

    std::fs::create_dir_all(paths::system_config_dir()).context("failed to create support dir")?;
    lines.push(match i18n.language() {
        Language::Es => format!("Directorio de soporte listo: {}", paths::APP_SUPPORT_DIR),
        Language::En => format!("Support directory ready: {}", paths::APP_SUPPORT_DIR),
    });

    migrate_or_create_config(config, &mut lines, i18n)?;

    remove_legacy_daemon().ok();
    write_plist(&plist_binary_path, config.daemon.interval_seconds)?;
    lines.push(match i18n.language() {
        Language::Es => format!("LaunchDaemon actualizado: {}", paths::PLIST_DEST),
        Language::En => format!("LaunchDaemon updated: {}", paths::PLIST_DEST),
    });

    bootstrap_launchd()?;
    lines.push(match i18n.language() {
        Language::Es => "Daemon cargado en launchd.".to_string(),
        Language::En => "Daemon loaded into launchd.".to_string(),
    });

    lines.push(if binary_path == PathBuf::from(paths::MANUAL_BINARY_DEST) {
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
    let plist_path = Path::new(paths::PLIST_DEST);

    if plist_path.exists() {
        let _ = Command::new("launchctl")
            .args(["bootout", "system", paths::PLIST_DEST])
            .output();
        std::fs::remove_file(plist_path).context("failed to remove plist")?;
        lines.push(match i18n.language() {
            Language::Es => "LaunchDaemon eliminado.".to_string(),
            Language::En => "LaunchDaemon removed.".to_string(),
        });
    }

    let manual_binary = Path::new(paths::MANUAL_BINARY_DEST);
    if manual_binary.exists() && !manual_binary.is_symlink() {
        std::fs::remove_file(manual_binary).context("failed to remove manual binary")?;
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
    std::fs::copy(&current_exe, &target).context("failed to copy binary")?;
    Command::new("chmod")
        .args(["755", paths::MANUAL_BINARY_DEST])
        .output()
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

fn migrate_or_create_config(
    config: &SystemConfig,
    lines: &mut Vec<String>,
    i18n: I18n,
) -> Result<()> {
    let active_config = paths::system_config_file();
    if active_config.exists() {
        lines.push(match i18n.language() {
            Language::Es => format!(
                "Se conserva la configuración existente: {}",
                active_config.display()
            ),
            Language::En => format!(
                "Keeping the existing configuration: {}",
                active_config.display()
            ),
        });
        return Ok(());
    }

    let legacy_path = Path::new(paths::LEGACY_SYSTEM_CONFIG_FILE);
    if legacy_path.exists() {
        let legacy_config = SystemConfig::load_from(legacy_path)?;
        legacy_config.save_active()?;
        lines.push(match i18n.language() {
            Language::Es => format!("Configuración migrada desde {}", legacy_path.display()),
            Language::En => format!("Configuration migrated from {}", legacy_path.display()),
        });
        return Ok(());
    }

    config.save_active()?;
    lines.push(match i18n.language() {
        Language::Es => format!("Configuración creada en {}", active_config.display()),
        Language::En => format!("Configuration created at {}", active_config.display()),
    });

    Ok(())
}

fn write_plist(binary_path: &str, interval_seconds: u32) -> Result<()> {
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>

	<key>ProgramArguments</key>
	<array>
		<string>{binary}</string>
		<string>run</string>
	</array>

	<key>RunAtLoad</key>
	<true/>

	<key>KeepAlive</key>
	<true/>

	<key>StandardOutPath</key>
	<string>/tmp/paramo.out</string>

	<key>StandardErrorPath</key>
	<string>/tmp/paramo.err</string>

	<key>StartInterval</key>
	<integer>{interval}</integer>

	<key>UserName</key>
	<string>root</string>

	<key>GroupName</key>
	<string>wheel</string>
</dict>
</plist>
"#,
        label = paths::LAUNCHD_LABEL,
        binary = binary_path,
        interval = interval_seconds
    );

    std::fs::write(paths::PLIST_DEST, plist).context("failed to write plist")?;
    Command::new("chown")
        .args(["root:wheel", paths::PLIST_DEST])
        .output()
        .context("failed to set plist owner")?;
    Command::new("chmod")
        .args(["644", paths::PLIST_DEST])
        .output()
        .context("failed to set plist permissions")?;
    Ok(())
}

fn bootstrap_launchd() -> Result<()> {
    let _ = Command::new("launchctl")
        .args(["bootout", "system", paths::PLIST_DEST])
        .output();

    Command::new("launchctl")
        .args(["bootstrap", "system", paths::PLIST_DEST])
        .output()
        .context("failed to bootstrap launchd")?;

    Ok(())
}

fn remove_legacy_daemon() -> Result<()> {
    let legacy_plist = Path::new(paths::LEGACY_PLIST_DEST);
    if legacy_plist.exists() {
        let _ = Command::new("launchctl")
            .args(["bootout", "system", paths::LEGACY_PLIST_DEST])
            .output();
    }

    Ok(())
}
