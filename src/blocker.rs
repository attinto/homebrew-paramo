use crate::config::SystemConfig;
use crate::hosts;
use crate::i18n::I18n;
use crate::scheduler;
use anyhow::Result;
use chrono::{DateTime, Datelike, Local};
use std::process::Command;
use tracing::{error, info};

#[derive(Debug, Clone, PartialEq)]
pub enum SyncAction {
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

#[derive(Debug, Clone)]
pub struct StatusSnapshot {
    pub now: DateTime<Local>,
    pub schedule_active: bool,
    pub hosts_blocked: bool,
    pub next_transition: Option<DateTime<Local>>,
    pub site_count: usize,
}

pub fn run(config: &SystemConfig) -> Result<SyncAction> {
    let now = Local::now();
    let should_block = scheduler::is_block_time(&config.schedule, &now);

    let hosts_path = &config.hosts.file;
    let content = hosts::read_hosts(hosts_path)?;

    let currently_blocked = hosts::is_blocked(&content, &config.hosts.marker);
    let block_section = hosts::build_block_section(
        &config.hosts.marker,
        &config.sites.list,
        &config.hosts.redirect_ips,
    );

    let block_is_current = hosts::has_expected_block(
        &content,
        &config.hosts.marker,
        &config.sites.list,
        &config.hosts.redirect_ips,
    )?;

    let action = if should_block {
        if !currently_blocked || !block_is_current {
            let action = if currently_blocked {
                SyncAction::Updated
            } else {
                SyncAction::Blocked
            };

            let mut new_content = if currently_blocked {
                hosts::remove_block(&content, &config.hosts.marker)?
            } else {
                content
            };

            new_content = hosts::add_block(&new_content, &block_section);
            hosts::write_hosts_atomic(hosts_path, &new_content)?;
            flush_dns();
            info!("Blocking state synced: {:?}", action);
            action
        } else {
            SyncAction::AlreadyCorrect(BlockState::Blocked)
        }
    } else if currently_blocked {
        let new_content = hosts::remove_block(&content, &config.hosts.marker)?;
        hosts::write_hosts_atomic(hosts_path, &new_content)?;
        flush_dns();
        info!("Blocking state synced: unblocked");
        SyncAction::Unblocked
    } else {
        SyncAction::AlreadyCorrect(BlockState::Unblocked)
    };

    Ok(action)
}

pub fn block_now(config: &SystemConfig) -> Result<()> {
    let hosts_path = &config.hosts.file;
    let content = hosts::read_hosts(hosts_path)?;
    let block_section = hosts::build_block_section(
        &config.hosts.marker,
        &config.sites.list,
        &config.hosts.redirect_ips,
    );

    let currently_blocked = hosts::is_blocked(&content, &config.hosts.marker);
    let new_content = if currently_blocked {
        let cleaned = hosts::remove_block(&content, &config.hosts.marker)?;
        hosts::add_block(&cleaned, &block_section)
    } else {
        hosts::add_block(&content, &block_section)
    };

    hosts::write_hosts_atomic(hosts_path, &new_content)?;
    flush_dns();
    info!("Manual block applied");
    Ok(())
}

pub fn unblock_now(config: &SystemConfig) -> Result<()> {
    let hosts_path = &config.hosts.file;
    let content = hosts::read_hosts(hosts_path)?;
    let new_content = hosts::remove_block(&content, &config.hosts.marker)?;
    hosts::write_hosts_atomic(hosts_path, &new_content)?;
    flush_dns();
    info!("Manual block removed");
    Ok(())
}

pub fn status_snapshot(config: &SystemConfig) -> Result<StatusSnapshot> {
    let now = Local::now();
    let content = hosts::read_hosts(&config.hosts.file)?;

    Ok(StatusSnapshot {
        now,
        schedule_active: scheduler::is_block_time(&config.schedule, &now),
        hosts_blocked: hosts::is_blocked(&content, &config.hosts.marker),
        next_transition: scheduler::next_transition(&config.schedule, &now),
        site_count: config.sites.list.len(),
    })
}

pub fn format_status(snapshot: &StatusSnapshot, i18n: I18n) -> String {
    let state = if snapshot.hosts_blocked {
        i18n.blocked_label()
    } else {
        i18n.unblocked_label()
    };
    let schedule = if snapshot.schedule_active {
        i18n.schedule_active_label()
    } else {
        i18n.schedule_inactive_label()
    };

    let mut status = format!(
        "{} {} | {} | {}",
        i18n.weekday(snapshot.now.weekday()),
        snapshot.now.format("%H:%M"),
        state,
        schedule
    );

    if let Some(next) = snapshot.next_transition {
        status.push_str(&format!(
            "\n{}: {} ({} {})",
            i18n.next_change_label(),
            next.format("%H:%M"),
            scheduler::format_duration_until(&next, &snapshot.now),
            i18n.relative_time_label()
        ));
    }

    status.push_str(&format!(
        "\n{}: {}",
        i18n.configured_sites_label(),
        snapshot.site_count
    ));

    status
}

fn flush_dns() {
    let is_root = unsafe { libc::geteuid() == 0 };

    let command = if is_root {
        "dscacheutil -flushcache && killall -HUP mDNSResponder 2>/dev/null || true"
    } else {
        "sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder 2>/dev/null || true"
    };

    if let Err(error) = Command::new("sh").arg("-c").arg(command).output() {
        error!("Failed to flush DNS: {}", error);
    }
}
