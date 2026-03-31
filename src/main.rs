mod blocker;
mod config;
mod hosts;
mod logging;
mod paths;
mod scheduler;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;
use tracing::info;

#[derive(Parser)]
#[command(name = "undistracted")]
#[command(about = "Bloqueador de distracciones para macOS", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Ejecuta el ciclo de bloqueo (usado por launchd)
    Run,

    /// Muestra el estado actual
    Status,

    /// Bloquea inmediatamente (requiere sudo)
    BlockNow,

    /// Desbloquea inmediatamente (requiere sudo)
    UnblockNow,

    /// Gestiona la configuración
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Instala el LaunchDaemon
    Install,

    /// Desinstala el LaunchDaemon
    Uninstall,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Muestra la configuración actual
    Show,

    /// Edita la configuración
    #[command(arg_required_else_help = true)]
    Set {
        /// Clave a modificar (ej: schedule.block_start)
        key: String,
        /// Valor nuevo
        value: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let config = Config::load().context("Failed to load configuration")?;
    let _logging_guard = logging::setup_logging(&config.logging.file, &config.logging.level)
        .context("Failed to setup logging")?;

    // Check if running as root (except for status and config show)
    let is_root = unsafe { libc::geteuid() == 0 };

    match cli.command {
        Some(Commands::Run) => {
            require_root()?;
            info!("Running blocker cycle...");
            let action = blocker::run(&config)?;
            info!("Action: {:?}", action);
        }

        Some(Commands::Status) => {
            let status = blocker::get_status(&config)?;
            println!("{}", status);
        }

        Some(Commands::BlockNow) => {
            require_root()?;
            blocker::block_now(&config)?;
            println!("✅ Bloqueado manualmente");
        }

        Some(Commands::UnblockNow) => {
            require_root()?;
            blocker::unblock_now(&config)?;
            println!("🔓 Desbloqueado manualmente");
        }

        Some(Commands::Config(config_cmd)) => {
            match config_cmd {
                ConfigCommands::Show => {
                    let content = match std::fs::read_to_string(paths::CONFIG_FILE) {
                        Ok(c) => c,
                        Err(_) => {
                            // Show default config
                            include_str!("../config/default.toml").to_string()
                        }
                    };
                    println!("{}", content);
                }

                ConfigCommands::Set { key, value } => {
                    require_root()?;
                    let mut config = Config::load().context("Failed to load configuration")?;
                    config
                        .update_value(&key, &value)
                        .map_err(|e| anyhow::anyhow!("❌ Error de configuración: {}", e))?;

                    config
                        .save(std::path::Path::new(paths::CONFIG_FILE))
                        .context("Failed to save configuration")?;

                    println!("✅ Configuración actualizada: {} = {}", key, value);
                }
            }
        }

        Some(Commands::Install) => {
            require_root()?;
            install_daemon(&config)?;
        }

        Some(Commands::Uninstall) => {
            require_root()?;
            uninstall_daemon()?;
        }

        None => {
            // Default: run the blocker cycle if root, otherwise show status
            if is_root {
                info!("Running blocker cycle (default)...");
                let action = blocker::run(&config)?;
                info!("Action: {:?}", action);
            } else {
                let status = blocker::get_status(&config)?;
                println!("{}", status);
            }
        }
    }

    Ok(())
}

fn require_root() -> Result<()> {
    if unsafe { libc::geteuid() != 0 } {
        anyhow::bail!(
            "❌ Este comando requiere permisos de administrador.\nEjecútalo con: sudo undistracted"
        );
    }
    Ok(())
}

fn install_daemon(config: &Config) -> Result<()> {
    println!("📦 Instalando Undistracted...");

    // Get the current binary path
    let binary_path = std::env::current_exe().context("Failed to get current binary path")?;

    // Create /etc/undistracted if it doesn't exist
    let config_dir = std::path::Path::new(paths::CONFIG_DIR);
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir).context("Failed to create config directory")?;
        println!("✅ Created {}", paths::CONFIG_DIR);
    }

    // Save default config if it doesn't exist
    let config_file = std::path::Path::new(paths::CONFIG_FILE);
    if !config_file.exists() {
        config.save(&config_file).context("Failed to save config")?;
        std::process::Command::new("chmod")
            .args(&["644", config_file.to_str().unwrap()])
            .output()
            .context("Failed to set config permissions")?;
        println!("✅ Created default config at {:?}", config_file);
    }

    // Copy binary to /usr/local/bin
    let target_path = std::path::Path::new(paths::BINARY_DEST);
    std::fs::copy(&binary_path, target_path).context("Failed to copy binary")?;
    std::process::Command::new("chmod")
        .args(&["755", target_path.to_str().unwrap()])
        .output()
        .context("Failed to set binary permissions")?;
    println!("✅ Installed binary at {:?}", target_path);

    // Copy plist
    let plist_content = include_str!("../launchd/com.undistracted.blocker.plist");
    let plist_path = std::path::Path::new(paths::PLIST_DEST);

    std::fs::write(plist_path, plist_content).context("Failed to write plist")?;
    std::process::Command::new("chown")
        .args(&["root:wheel", plist_path.to_str().unwrap()])
        .output()
        .context("Failed to set plist ownership")?;
    std::process::Command::new("chmod")
        .args(&["644", plist_path.to_str().unwrap()])
        .output()
        .context("Failed to set plist permissions")?;
    println!("✅ Installed LaunchDaemon plist");

    // Load the daemon
    std::process::Command::new("launchctl")
        .args(&["bootstrap", "system", plist_path.to_str().unwrap()])
        .output()
        .context("Failed to bootstrap LaunchDaemon")?;

    println!("✅ LaunchDaemon bootstrapped");
    println!("✨ Instalación completada. Undistracted está activo.");

    Ok(())
}

fn uninstall_daemon() -> Result<()> {
    println!("🗑️  Desinstalando Undistracted...");

    // Unload the daemon
    let plist_path = paths::PLIST_DEST;
    if std::path::Path::new(plist_path).exists() {
        let _ = std::process::Command::new("launchctl")
            .args(&["bootout", "system", plist_path])
            .output();
        println!("✅ LaunchDaemon desactivado");

        std::fs::remove_file(plist_path).context("Failed to remove plist")?;
        println!("✅ Eliminado plist");
    }

    // Remove binary
    if std::path::Path::new(paths::BINARY_DEST).exists() {
        std::fs::remove_file(paths::BINARY_DEST).context("Failed to remove binary")?;
        println!("✅ Eliminado binario");
    }

    println!("✨ Desinstalación completada.");

    Ok(())
}
