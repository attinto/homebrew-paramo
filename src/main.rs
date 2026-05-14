mod attempts;
mod blocker;
mod config;
mod doctor;
mod hosts;
mod i18n;
mod install;
mod ipc;
mod journal;
mod launchd;
mod logging;
mod paths;
mod preferences;
mod scheduler;
mod streak;
mod tui;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use config::{SiteMutation, SystemConfig};
use i18n::{I18n, Language};
use preferences::UserPreferences;
use std::io::{BufRead, IsTerminal, Write};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "paramo")]
#[command(version)]
#[command(about = "PARAMO: bloqueador de distracciones con CLI y TUI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(hide = true)]
    Run,
    Status,
    Block,
    Unblock,
    Doctor,
    Install,
    Uninstall,
    #[command(subcommand)]
    Site(SiteCommand),
    #[command(subcommand)]
    Schedule(ScheduleCommand),
    #[command(subcommand)]
    Lang(LanguageCommand),
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Subcommand)]
enum SiteCommand {
    List,
    Add { site: String },
    Remove { site: String },
}

#[derive(Subcommand)]
enum ScheduleCommand {
    Show,
    Set(ScheduleSetArgs),
}

#[derive(Args)]
struct ScheduleSetArgs {
    #[arg(long)]
    start: u8,
    #[arg(long)]
    end: u8,
    #[arg(long, value_enum, default_value_t = ToggleOption::Off)]
    weekends: ToggleOption,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ToggleOption {
    On,
    Off,
}

impl ToggleOption {
    fn as_bool(self) -> bool {
        matches!(self, Self::On)
    }
}

#[derive(Subcommand)]
enum LanguageCommand {
    Show,
    Set { language: String },
}

#[derive(Subcommand)]
enum ConfigCommand {
    Show,
}

fn main() -> Result<()> {
    let mut preferences = UserPreferences::load().context("Failed to load user preferences")?;
    let mut config = SystemConfig::load().context("Failed to load system configuration")?;
    let mut i18n = I18n::new(preferences.language);
    let _logging_guard = logging::setup_logging(&config.logging.file, &config.logging.level)
        .context("Failed to setup logging")?;
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run) => {
            require_root(i18n)?;
            blocker::run_daemon(&config)?;
        }
        Some(Command::Status) => {
            print_status(&config, i18n)?;
        }
        Some(Command::Block) => {
            ipc::send_command("block").map_err(anyhow::Error::msg)?;
            println!("{}", i18n.blocked_now());
        }
        Some(Command::Unblock) => {
            ipc::send_command("unblock").map_err(anyhow::Error::msg)?;
            println!("{}", i18n.unblocked_now());
        }
        Some(Command::Doctor) => {
            let diagnostics = doctor::run(&config, i18n)?;
            println!("{}", doctor::render_cli(&diagnostics, i18n));
        }
        Some(Command::Install) => {
            require_root(i18n)?;
            let summary = install::install(&config, i18n)?;
            for line in summary.lines {
                println!("{}", line);
            }
        }
        Some(Command::Uninstall) => {
            require_root(i18n)?;
            let summary = install::uninstall(i18n)?;
            for line in summary.lines {
                println!("{}", line);
            }
        }
        Some(Command::Site(command)) => match command {
            SiteCommand::List => {
                if config.sites.list.is_empty() {
                    println!("{}", i18n.site_empty());
                } else {
                    for site in &config.sites.list {
                        println!("{}", site);
                    }
                }
            }
            SiteCommand::Add { site } => {
                match config.add_site(&site).map_err(anyhow::Error::msg)? {
                    SiteMutation::Added(site) => {
                        config.save_active()?;
                        ipc::send_command("sync").map_err(anyhow::Error::msg)?;
                        println!("{}", i18n.site_added(&site));
                    }
                    SiteMutation::AlreadyPresent(site) => {
                        println!("{}", i18n.site_already_present(&site));
                    }
                    _ => {}
                }
            }
            SiteCommand::Remove { site } => {
                cli_remove_site_with_friction(&mut config, &site, i18n)?;
            }
        },
        Some(Command::Schedule(command)) => match command {
            ScheduleCommand::Show => {
                println!(
                    "{}",
                    i18n.schedule_summary(
                        config.schedule.start,
                        config.schedule.end,
                        config.schedule.block_weekends
                    )
                );
            }
            ScheduleCommand::Set(args) => {
                config
                    .set_schedule(args.start, args.end, args.weekends.as_bool())
                    .map_err(anyhow::Error::msg)?;
                config.save_active()?;
                ipc::send_command("sync").map_err(anyhow::Error::msg)?;
                println!(
                    "{}",
                    i18n.schedule_updated(
                        config.schedule.start,
                        config.schedule.end,
                        config.schedule.block_weekends
                    )
                );
            }
        },
        Some(Command::Lang(command)) => match command {
            LanguageCommand::Show => {
                println!("{}", i18n.current_language(preferences.language));
            }
            LanguageCommand::Set { language } => {
                let language = Language::parse(&language)
                    .ok_or_else(|| anyhow::anyhow!(i18n.unsupported_language(&language)))?;
                preferences.language = language;
                preferences.save()?;
                i18n = I18n::new(language);
                println!("{}", i18n.language_updated(language));
            }
        },
        Some(Command::Config(command)) => match command {
            ConfigCommand::Show => {
                let content = SystemConfig::load_effective_contents()?;
                println!("{}", content);
            }
        },
        None => {
            if std::io::stdout().is_terminal() {
                tui::run(&mut config, &mut preferences)?;
            } else {
                print_status(&config, i18n)?;
            }
        }
    }

    Ok(())
}

fn print_status(config: &SystemConfig, i18n: I18n) -> Result<()> {
    let snapshot = blocker::status_snapshot(config)?;
    println!("{}", blocker::format_status(&snapshot, i18n));
    Ok(())
}

fn require_root(i18n: I18n) -> Result<()> {
    if unsafe { libc::geteuid() != 0 } {
        anyhow::bail!("{}", i18n.requires_root());
    }

    Ok(())
}

// Quitar un sitio desde el CLI replica el mismo proceso de fricción que la
// TUI (30 s + motivo obligatorio + 60 s) para que no haya un atajo trivial
// que sortee la protección.
fn cli_remove_site_with_friction(
    config: &mut SystemConfig,
    raw_site: &str,
    i18n: I18n,
) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("{}", i18n.t("site_remove_requires_tty"));
    }

    // Validamos antes de empezar la espera: si el sitio no está en la lista,
    // no tiene sentido hacer al usuario esperar 90 segundos.
    let normalized = config::normalize_site_input(raw_site).map_err(anyhow::Error::msg)?;
    if !config
        .sites
        .list
        .iter()
        .any(|existing| config::normalize_site_input(existing).ok().as_deref() == Some(&normalized))
    {
        println!("{}", i18n.site_not_found(&normalized));
        return Ok(());
    }

    println!("{}", i18n.format("site_remove_initiated", &[&normalized]));

    println!("{}", i18n.t("site_remove_phase_first"));
    cli_countdown(30)?;

    let reason = cli_prompt_reason(i18n)?;

    println!("{}", i18n.t("site_remove_phase_final"));
    cli_countdown(60)?;

    match config.remove_site(&normalized).map_err(anyhow::Error::msg)? {
        SiteMutation::Removed(site) => {
            config.save_active()?;
            let _ = journal::append_site_removal(&site, &reason);
            // El daemon puede no estar arrancado en entornos de desarrollo;
            // si no responde, lo informamos pero no abortamos.
            if let Err(error) = ipc::send_command("sync") {
                eprintln!("{}", error);
            }
            println!(
                "{}",
                i18n.format("site_remove_completed", &[&site, &reason])
            );
        }
        SiteMutation::NotFound(site) => println!("{}", i18n.site_not_found(&site)),
        _ => {}
    }

    Ok(())
}

fn cli_countdown(seconds: u64) -> Result<()> {
    let mut stdout = std::io::stdout();
    let start = Instant::now();
    let total = Duration::from_secs(seconds);
    while start.elapsed() < total {
        let elapsed = start.elapsed().as_secs();
        let remaining = seconds.saturating_sub(elapsed);
        write!(stdout, "\r  {:>3}s ", remaining)?;
        stdout.flush()?;
        std::thread::sleep(Duration::from_millis(1000));
    }
    writeln!(stdout, "\r  {:>3}s ", 0)?;
    stdout.flush()?;
    Ok(())
}

fn cli_prompt_reason(i18n: I18n) -> Result<String> {
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    loop {
        println!("{}", i18n.t("site_remove_phase_reason"));
        let mut line = String::new();
        handle.read_line(&mut line)?;
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        println!("{}", i18n.reason_required());
    }
}
