use crate::config::{ConfigSource, SystemConfig};
use crate::hosts;
use crate::i18n::{I18n, Language};
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
            i18n,
            text(
                i18n,
                "Configuración activa encontrada",
                "Active configuration found",
            ),
            paths::SYSTEM_CONFIG_FILE,
            None,
        ),
        ConfigSource::Legacy => diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            text(
                i18n,
                "PARAMO está usando la configuración legacy",
                "PARAMO is using the legacy configuration",
            ),
            text(
                i18n,
                "No existe /etc/paramo/config.toml; la app está leyendo /etc/undistracted/config.toml.",
                "No /etc/paramo/config.toml exists; the app is reading /etc/undistracted/config.toml.",
            ),
            Some(text(
                i18n,
                "Ejecuta `sudo paramo install` para migrarla.",
                "Run `sudo paramo install` to migrate it.",
            )),
        ),
        ConfigSource::EmbeddedDefault => diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            text(
                i18n,
                "No existe configuración instalada",
                "No installed configuration was found",
            ),
            text(
                i18n,
                "PARAMO está usando la plantilla embebida del repositorio; todavía no hay /etc/paramo/config.toml.",
                "PARAMO is using the embedded template from the repo; /etc/paramo/config.toml does not exist yet.",
            ),
            Some(text(
                i18n,
                "Ejecuta `sudo paramo install` para crear la configuración activa.",
                "Run `sudo paramo install` to create the active configuration.",
            )),
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
        i18n,
        text(
            i18n,
            "Se han detectado restos de Undistracted",
            "Legacy Undistracted artifacts were detected",
        ),
        artifacts.join(", "),
        Some(text(
            i18n,
            "PARAMO puede funcionar así, pero conviene limpiar esos restos para evitar confusión.",
            "PARAMO can work like this, but removing those leftovers is recommended to avoid confusion.",
        )),
    )))
}

fn plist_diagnostic(config: &SystemConfig, i18n: I18n) -> Diagnostic {
    let plist_path = Path::new(paths::PLIST_DEST);
    if !plist_path.exists() {
        return diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            text(
                i18n,
                "LaunchDaemon no instalado",
                "LaunchDaemon not installed",
            ),
            paths::PLIST_DEST,
            Some(text(
                i18n,
                "Sin daemon, PARAMO solo actuará cuando lo ejecutes manualmente.",
                "Without the daemon, PARAMO only runs when launched manually.",
            )),
        );
    }

    let content = match std::fs::read_to_string(plist_path) {
        Ok(content) => content,
        Err(error) => {
            return diagnostic(
                DiagnosticLevel::Error,
                i18n,
                text(i18n, "No se pudo leer el plist", "Could not read the plist"),
                error.to_string(),
                None,
            );
        }
    };

    let label = launchd::plist_value(&content, "Label");
    let interval = launchd::plist_integer(&content, "StartInterval");
    let args = launchd::plist_program_arguments(&content);
    let binary = args.first().cloned();
    let run_argument_ok = args.get(1).is_some_and(|value| value == "run");

    if label.as_deref() != Some(paths::LAUNCHD_LABEL)
        || interval != Some(config.daemon.interval_seconds)
        || !run_argument_ok
    {
        return diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            text(
                i18n,
                "El plist de launchd no coincide con la configuración actual",
                "The launchd plist does not match the current configuration",
            ),
            format!(
                "Label={:?}, StartInterval={:?}, ProgramArguments={:?}",
                label, interval, args
            ),
            Some(text(
                i18n,
                "Ejecuta `sudo paramo install` para regenerarlo.",
                "Run `sudo paramo install` to regenerate it.",
            )),
        );
    }

    if let Some(binary) = binary {
        if !Path::new(&binary).exists() {
            return diagnostic(
                DiagnosticLevel::Error,
                i18n,
                text(
                    i18n,
                    "El plist apunta a un binario inexistente",
                    "The plist points to a missing binary",
                ),
                binary,
                Some(text(
                    i18n,
                    "Ejecuta `sudo paramo install` para reparar la instalación.",
                    "Run `sudo paramo install` to repair the installation.",
                )),
            );
        }
    }

    diagnostic(
        DiagnosticLevel::Ok,
        i18n,
        text(i18n, "LaunchDaemon válido", "LaunchDaemon looks valid"),
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
            i18n,
            text(
                i18n,
                "Servicio cargado en launchd",
                "Service loaded in launchd",
            ),
            detail,
            None,
        ));
    }

    let (detail, hint) = match service.disabled {
        Some(true) => (
            text(
                i18n,
                "El servicio existe pero está marcado como disabled en launchd.",
                "The service exists but it is marked as disabled in launchd.",
            ),
            text(
                i18n,
                "Ejecuta `sudo paramo install` para volver a registrarlo.",
                "Run `sudo paramo install` to register it again.",
            ),
        ),
        _ => (
            text(
                i18n,
                "Launchd no tiene cargado el servicio de PARAMO.",
                "launchd does not have the PARAMO service loaded.",
            ),
            text(
                i18n,
                "Ejecuta `sudo paramo install` y vuelve a comprobarlo con `paramo doctor`.",
                "Run `sudo paramo install` and check again with `paramo doctor`.",
            ),
        ),
    };

    Ok(diagnostic(
        DiagnosticLevel::Warn,
        i18n,
        text(
            i18n,
            "Servicio no cargado en launchd",
            "Service is not loaded in launchd",
        ),
        detail,
        Some(hint),
    ))
}

fn site_list_diagnostic(config: &SystemConfig, i18n: I18n) -> Diagnostic {
    if config.sites.list.is_empty() {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            text(i18n, "No hay sitios configurados", "No sites configured"),
            text(
                i18n,
                "La lista de sitios está vacía.",
                "The site list is empty.",
            ),
            Some(text(
                i18n,
                "Añade sitios con `paramo site add dominio.com`.",
                "Add sites with `paramo site add domain.com`.",
            )),
        )
    } else {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n,
            text(i18n, "Lista de sitios cargada", "Site list loaded"),
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
                i18n,
                text(
                    i18n,
                    "No se pudo leer el archivo hosts",
                    "Could not read the hosts file",
                ),
                error.to_string(),
                Some(text(
                    i18n,
                    "Comprueba que el archivo exista y que PARAMO tenga permisos para leerlo.",
                    "Check that the file exists and PARAMO can read it.",
                )),
            ));
            return Ok(diagnostics);
        }
    };

    let marker_count = hosts::count_markers(&hosts_content, &config.hosts.marker);
    diagnostics.push(if marker_count > 2 {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            text(
                i18n,
                "Secciones duplicadas en hosts",
                "Duplicate hosts sections detected",
            ),
            format!("{} markers", marker_count),
            Some(text(
                i18n,
                "Ejecuta `sudo paramo block` o `sudo paramo unblock` para normalizar el bloque.",
                "Run `sudo paramo block` or `sudo paramo unblock` to normalize the block.",
            )),
        )
    } else {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n,
            text(
                i18n,
                "Archivo hosts consistente",
                "Hosts file looks consistent",
            ),
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
                i18n,
                text(
                    i18n,
                    "El bloque activo no coincide con la configuración",
                    "Active hosts block does not match the configuration",
                ),
                text(
                    i18n,
                    "Hay una discrepancia entre `/etc/hosts` y la lista de sitios configurados.",
                    "There is a mismatch between `/etc/hosts` and the configured site list.",
                ),
                Some(text(
                    i18n,
                    "Ejecuta `sudo paramo run` para resincronizar el bloqueo.",
                    "Run `sudo paramo run` to resync blocking.",
                )),
            )
        } else {
            diagnostic(
                DiagnosticLevel::Ok,
                i18n,
                text(
                    i18n,
                    "Bloque hosts sincronizado",
                    "Hosts block is synchronized",
                ),
                text(
                    i18n,
                    "La sección gestionada por PARAMO coincide con la configuración.",
                    "The PARAMO-managed section matches the configuration.",
                ),
                None,
            )
        },
    );

    Ok(diagnostics)
}

fn doh_diagnostic(i18n: I18n) -> Diagnostic {
    diagnostic(
        DiagnosticLevel::Warn,
        i18n,
        text(
            i18n,
            "Recuerda revisar DNS over HTTPS en navegadores",
            "Remember to review DNS over HTTPS in browsers",
        ),
        text(
            i18n,
            "Firefox y otros navegadores pueden ignorar `/etc/hosts` si DoH está activo.",
            "Firefox and other browsers can ignore `/etc/hosts` when DoH is enabled.",
        ),
        Some(text(
            i18n,
            "Si una web sigue entrando, desactiva DoH en el navegador.",
            "If a site still opens, disable DoH in the browser.",
        )),
    )
}

fn text<'a>(i18n: I18n, spanish: &'a str, english: &'a str) -> &'a str {
    match i18n.language() {
        Language::Es => spanish,
        Language::En => english,
    }
}

fn diagnostic(
    level: DiagnosticLevel,
    _i18n: I18n,
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

    #[test]
    fn test_render_cli_contains_levels() {
        let diagnostics = vec![diagnostic(
            DiagnosticLevel::Warn,
            I18n::new(Language::En),
            "Title",
            "Detail",
            Some("Hint"),
        )];
        let rendered = render_cli(&diagnostics, I18n::new(Language::En));
        assert!(rendered.contains("[WARN] Title"));
    }
}
