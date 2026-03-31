use crate::config::Config;
use crate::hosts;
use crate::scheduler;
use anyhow::Result;
use chrono::{Datelike, Local};
use std::process::Command;
use tracing::{error, info};

#[derive(Debug, Clone, PartialEq)]
pub enum BlockerAction {
    Blocked,
    Unblocked,
    Updated,
    AlreadyCorrect(BlockState),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    Blocked,
    Unblocked,
}

pub fn run(config: &Config) -> Result<BlockerAction> {
    let now = Local::now();
    let should_block = scheduler::is_block_time(&config.schedule, &now);

    let hosts_path = &config.hosts.file;
    let content = hosts::read_hosts(hosts_path)?;

    let currently_blocked = hosts::is_blocked(&content, &config.hosts.marker);
    let block_section = hosts::build_block_section(
        &config.hosts.marker,
        &config.domains.list,
        &config.hosts.redirect_ips,
    );

    let block_is_current = hosts::has_expected_block(
        &content,
        &config.hosts.marker,
        &config.domains.list,
        &config.hosts.redirect_ips,
    )?;

    let action = if should_block {
        if !currently_blocked || !block_is_current {
            let action = if currently_blocked {
                BlockerAction::Updated
            } else {
                BlockerAction::Blocked
            };

            let mut new_content = if currently_blocked {
                hosts::remove_block(&content, &config.hosts.marker)?
            } else {
                content
            };

            new_content = hosts::add_block(&new_content, &block_section);
            hosts::write_hosts_atomic(hosts_path, &new_content)?;

            flush_dns();

            info!(
                "✅ BLOQUEADO — Sitios configurados bloqueados hasta las {}:00h",
                config.schedule.block_end
            );
            action
        } else {
            BlockerAction::AlreadyCorrect(BlockState::Blocked)
        }
    } else if currently_blocked {
        let new_content = hosts::remove_block(&content, &config.hosts.marker)?;
        hosts::write_hosts_atomic(hosts_path, &new_content)?;

        flush_dns();

        info!(
            "🔓 DESBLOQUEADO — Sitios configurados disponibles hasta las {}:00h",
            config.schedule.block_start
        );
        BlockerAction::Unblocked
    } else {
        BlockerAction::AlreadyCorrect(BlockState::Unblocked)
    };

    Ok(action)
}

fn flush_dns() {
    // Flush macOS DNS cache
    // If we're already root (which we should be), we don't need sudo inside the command
    let is_root = unsafe { libc::geteuid() == 0 };

    let cmd = if is_root {
        "dscacheutil -flushcache && killall -HUP mDNSResponder 2>/dev/null || true"
    } else {
        "sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder 2>/dev/null || true"
    };

    if let Err(e) = Command::new("sh").arg("-c").arg(cmd).output() {
        error!("Failed to flush DNS: {}", e);
    }
}

pub fn block_now(config: &Config) -> Result<()> {
    let hosts_path = &config.hosts.file;
    let content = hosts::read_hosts(hosts_path)?;

    let block_section = hosts::build_block_section(
        &config.hosts.marker,
        &config.domains.list,
        &config.hosts.redirect_ips,
    );

    let currently_blocked = hosts::is_blocked(&content, &config.hosts.marker);

    let new_content = if currently_blocked {
        let temp = hosts::remove_block(&content, &config.hosts.marker)?;
        hosts::add_block(&temp, &block_section)
    } else {
        hosts::add_block(&content, &block_section)
    };

    hosts::write_hosts_atomic(hosts_path, &new_content)?;
    flush_dns();
    info!("🛑 Bloqueado manualmente");

    Ok(())
}

pub fn unblock_now(config: &Config) -> Result<()> {
    let hosts_path = &config.hosts.file;
    let content = hosts::read_hosts(hosts_path)?;

    let new_content = hosts::remove_block(&content, &config.hosts.marker)?;
    hosts::write_hosts_atomic(hosts_path, &new_content)?;
    flush_dns();
    info!("🔓 Desbloqueado manualmente");

    Ok(())
}

pub fn get_status(config: &Config) -> Result<String> {
    let now = Local::now();
    let should_block = scheduler::is_block_time(&config.schedule, &now);

    let hosts_path = &config.hosts.file;
    let content = hosts::read_hosts(hosts_path)?;
    let currently_blocked = hosts::is_blocked(&content, &config.hosts.marker);

    let next_transition = scheduler::next_transition(&config.schedule, &now);

    let day_name = [
        "Lunes",
        "Martes",
        "Miércoles",
        "Jueves",
        "Viernes",
        "Sábado",
        "Domingo",
    ][now.weekday().number_from_monday() as usize - 1];

    let time_str = now.format("%H:%M").to_string();
    let state_str = if currently_blocked {
        "🔴 BLOQUEADO"
    } else {
        "🟢 DESBLOQUEADO"
    };

    let mut status = format!(
        "{} {} | {} | {}",
        day_name,
        time_str,
        state_str,
        if should_block {
            "Horario de bloqueo activo"
        } else {
            "Fuera del horario de bloqueo"
        }
    );

    if let Some(next) = next_transition {
        let time_until = scheduler::format_duration_until(&next, &now);
        let next_time = next.format("%H:%M").to_string();
        status.push_str(&format!(
            "\nPróximo cambio: {} ({} desde ahora)",
            next_time, time_until
        ));
    }

    Ok(status)
}
