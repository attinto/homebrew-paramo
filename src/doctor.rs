use crate::config::{ConfigSource, SystemConfig};
use crate::hosts;
use crate::i18n::I18n;
use crate::launchd;
use crate::paths;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub title: String,
    pub detail: String,
    pub hint: Option<String>,
}

pub fn run(config: &SystemConfig, i18n: I18n) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    diagnostics.push(config_source_diagnostic(i18n));

    if let Some(legacy) = legacy_installation_diagnostic(i18n)? {
        diagnostics.push(legacy);
    }

    diagnostics.push(plist_diagnostic(config, i18n));
    diagnostics.push(launchd_runtime_diagnostic(i18n)?);
    diagnostics.push(site_list_diagnostic(config, i18n));
    diagnostics.extend(hosts_diagnostics(config, i18n)?);
    diagnostics.push(doh_diagnostic(i18n));

    Ok(diagnostics)
}

pub fn render_cli(diagnostics: &[Diagnostic], i18n: I18n) -> String {
    let mut lines = vec![i18n.doctor_title().to_string()];

    for item in diagnostics {
        let level = match item.level {
            DiagnosticLevel::Ok => i18n.ok(),
            DiagnosticLevel::Warn => i18n.warning(),
            DiagnosticLevel::Error => i18n.error(),
        };

        lines.push(format!("[{}] {}", level, item.title));
        lines.push(format!("  {}", item.detail));
        if let Some(hint) = &item.hint {
            lines.push(format!("  {}", hint));
        }
    }

    lines.join("\n")
}

fn config_source_diagnostic(i18n: I18n) -> Diagnostic {
    match SystemConfig::config_source() {
        ConfigSource::Active => diagnostic(
            DiagnosticLevel::Ok,
            i18n.t("doctor_config_active_title"),
            paths::SYSTEM_CONFIG_FILE,
            None,
        ),
        ConfigSource::Legacy => diagnostic(
            DiagnosticLevel::Warn,
            i18n.t("doctor_config_legacy_title"),
            i18n.t("doctor_config_legacy_detail"),
            Some(i18n.t("doctor_config_legacy_hint")),
        ),
        ConfigSource::EmbeddedDefault => diagnostic(
            DiagnosticLevel::Warn,
            i18n.t("doctor_config_embedded_title"),
            i18n.t("doctor_config_embedded_detail"),
            Some(i18n.t("doctor_config_embedded_hint")),
        ),
    }
}

fn legacy_installation_diagnostic(i18n: I18n) -> Result<Option<Diagnostic>> {
    let mut artifacts = Vec::new();

    if SystemConfig::config_source() == ConfigSource::Legacy
        && Path::new(paths::LEGACY_SYSTEM_CONFIG_FILE).exists()
    {
        artifacts.push(paths::LEGACY_SYSTEM_CONFIG_FILE.to_string());
    }
    if Path::new(paths::LEGACY_PLIST_DEST).exists() {
        artifacts.push(paths::LEGACY_PLIST_DEST.to_string());
    }

    let legacy_service = launchd::query_service(paths::LEGACY_LAUNCHD_LABEL)?;
    if legacy_service.loaded {
        artifacts.push(format!("launchd:{}", paths::LEGACY_LAUNCHD_LABEL));
    } else if legacy_service.disabled == Some(true) {
        artifacts.push(format!("launchd-disabled:{}", paths::LEGACY_LAUNCHD_LABEL));
    }

    if artifacts.is_empty() {
        return Ok(None);
    }

    Ok(Some(diagnostic(
        DiagnosticLevel::Warn,
        i18n.t("doctor_legacy_artifacts_title"),
        artifacts.join(", "),
        Some(i18n.t("doctor_legacy_artifacts_hint")),
    )))
}

fn plist_diagnostic(_config: &SystemConfig, i18n: I18n) -> Diagnostic {
    let plist_path = Path::new(paths::PLIST_DEST);
    if !plist_path.exists() {
        return diagnostic(
            DiagnosticLevel::Warn,
            i18n.t("doctor_plist_missing_title"),
            paths::PLIST_DEST,
            Some(i18n.t("doctor_plist_missing_hint")),
        );
    }

    let content = match std::fs::read_to_string(plist_path) {
        Ok(content) => content,
        Err(error) => {
            return diagnostic(
                DiagnosticLevel::Error,
                i18n.t("doctor_plist_unreadable_title"),
                error.to_string(),
                None,
            );
        }
    };

    let label = launchd::plist_value(&content, "Label");
    let args = launchd::plist_program_arguments(&content);
    let binary = args.first().cloned();
    let run_argument_ok = args.get(1).is_some_and(|value| value == "run");

    if label.as_deref() != Some(paths::LAUNCHD_LABEL) || !run_argument_ok {
        return diagnostic(
            DiagnosticLevel::Warn,
            i18n.t("doctor_plist_mismatch_title"),
            format!("Label={:?}, ProgramArguments={:?}", label, args),
            Some(i18n.t("doctor_plist_mismatch_hint")),
        );
    }

    if let Some(binary) = binary {
        if !Path::new(&binary).exists() {
            return diagnostic(
                DiagnosticLevel::Error,
                i18n.t("doctor_plist_binary_missing_title"),
                binary,
                Some(i18n.t("doctor_plist_binary_missing_hint")),
            );
        }
    }

    diagnostic(
        DiagnosticLevel::Ok,
        i18n.t("doctor_plist_valid_title"),
        format!("{} -> {:?}", paths::PLIST_DEST, args),
        None,
    )
}

fn launchd_runtime_diagnostic(i18n: I18n) -> Result<Diagnostic> {
    let service = launchd::query_service(paths::LAUNCHD_LABEL)?;

    if service.loaded {
        let mut detail = paths::LAUNCHD_LABEL.to_string();
        if let Some(program) = service.program {
            detail.push_str(&format!(" | {}", program.display()));
        }
        if let Some(pid) = service.pid {
            detail.push_str(&format!(" | pid {pid}"));
        }
        if let Some(exit) = service.last_exit_status {
            detail.push_str(&format!(" | last exit {exit}"));
        }

        return Ok(diagnostic(
            DiagnosticLevel::Ok,
            i18n.t("doctor_launchd_loaded_title"),
            detail,
            None,
        ));
    }

    let (detail, hint) = match service.disabled {
        Some(true) => (
            i18n.t("doctor_launchd_disabled_detail"),
            i18n.t("doctor_launchd_disabled_hint"),
        ),
        _ => (
            i18n.t("doctor_launchd_not_loaded_detail"),
            i18n.t("doctor_launchd_not_loaded_hint"),
        ),
    };

    Ok(diagnostic(
        DiagnosticLevel::Warn,
        i18n.t("doctor_launchd_not_loaded_title"),
        detail,
        Some(hint),
    ))
}

fn site_list_diagnostic(config: &SystemConfig, i18n: I18n) -> Diagnostic {
    if config.sites.list.is_empty() {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n.t("doctor_sites_empty_title"),
            i18n.t("doctor_sites_empty_detail"),
            Some(i18n.t("doctor_sites_empty_hint")),
        )
    } else {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n.t("doctor_sites_loaded_title"),
            format!(
                "{}: {}",
                i18n.configured_sites_label(),
                config.sites.list.len()
            ),
            None,
        )
    }
}

fn hosts_diagnostics(config: &SystemConfig, i18n: I18n) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let hosts_content = match hosts::read_hosts(&config.hosts.file) {
        Ok(content) => content,
        Err(error) => {
            diagnostics.push(diagnostic(
                DiagnosticLevel::Error,
                i18n.t("doctor_hosts_unreadable_title"),
                error.to_string(),
                Some(i18n.t("doctor_hosts_unreadable_hint")),
            ));
            return Ok(diagnostics);
        }
    };

    let marker_count = hosts::count_markers(&hosts_content, &config.hosts.marker);
    diagnostics.push(if marker_count > 2 {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n.t("doctor_hosts_duplicate_title"),
            format!("{} markers", marker_count),
            Some(i18n.t("doctor_hosts_duplicate_hint")),
        )
    } else {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n.t("doctor_hosts_consistent_title"),
            config.hosts.file.display().to_string(),
            None,
        )
    });

    diagnostics.push(
        if hosts::is_blocked(&hosts_content, &config.hosts.marker)
            && !hosts::has_expected_block(
                &hosts_content,
                &config.hosts.marker,
                &config.sites.list,
                &config.hosts.redirect_ips,
            )?
        {
            diagnostic(
                DiagnosticLevel::Warn,
                i18n.t("doctor_hosts_mismatch_title"),
                i18n.t("doctor_hosts_mismatch_detail"),
                Some(i18n.t("doctor_hosts_mismatch_hint")),
            )
        } else {
            diagnostic(
                DiagnosticLevel::Ok,
                i18n.t("doctor_hosts_synced_title"),
                i18n.t("doctor_hosts_synced_detail"),
                None,
            )
        },
    );

    Ok(diagnostics)
}

fn doh_diagnostic(i18n: I18n) -> Diagnostic {
    diagnostic(
        DiagnosticLevel::Warn,
        i18n.t("doctor_doh_title"),
        i18n.t("doctor_doh_detail"),
        Some(i18n.t("doctor_doh_hint")),
    )
}

fn diagnostic(
    level: DiagnosticLevel,
    title: impl Into<String>,
    detail: impl Into<String>,
    hint: Option<&str>,
) -> Diagnostic {
    Diagnostic {
        level,
        title: title.into(),
        detail: detail.into(),
        hint: hint.map(|value| value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn test_render_cli_contains_levels() {
        let diagnostics = vec![diagnostic(
            DiagnosticLevel::Warn,
            "Title",
            "Detail",
            Some("Hint"),
        )];
        let rendered = render_cli(&diagnostics, I18n::new(Language::En));
        assert!(rendered.contains("[WARN] Title"));
    }
}
