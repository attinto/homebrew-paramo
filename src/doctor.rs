use crate::config::SystemConfig;
use crate::hosts;
use crate::i18n::{I18n, Language};
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
    let system_config_path = paths::system_config_file();
    let legacy_config_path = Path::new(paths::LEGACY_SYSTEM_CONFIG_FILE);
    let plist_path = Path::new(paths::PLIST_DEST);
    let legacy_plist_path = Path::new(paths::LEGACY_PLIST_DEST);

    diagnostics.push(if system_config_path.exists() {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n,
            match i18n.language() {
                Language::Es => "Configuración activa encontrada",
                Language::En => "Active configuration found",
            },
            system_config_path.display().to_string(),
            None,
        )
    } else {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            match i18n.language() {
                Language::Es => "No existe configuración activa",
                Language::En => "Active configuration is missing",
            },
            system_config_path.display().to_string(),
            Some(match i18n.language() {
                Language::Es => "Ejecuta `sudo paramo install` para crearla.",
                Language::En => "Run `sudo paramo install` to create it.",
            }),
        )
    });

    if legacy_config_path.exists()
        || legacy_plist_path.exists()
        || Path::new(paths::LEGACY_BINARY_DEST).exists()
    {
        diagnostics.push(diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            match i18n.language() {
                Language::Es => "Se ha detectado una instalación antigua de Undistracted",
                Language::En => "A legacy Undistracted installation was detected",
            },
            match i18n.language() {
                Language::Es => "PARAMO puede migrar la configuración, pero conviene retirar la instalación vieja.",
                Language::En => "PARAMO can migrate the configuration, but removing the old installation is recommended.",
            }
            .to_string(),
            Some(match i18n.language() {
                Language::Es => "Revisa `/etc/undistracted` y `/Library/LaunchDaemons/com.undistracted.blocker.plist`.",
                Language::En => "Review `/etc/undistracted` and `/Library/LaunchDaemons/com.undistracted.blocker.plist`.",
            }),
        ));
    }

    diagnostics.push(if plist_path.exists() {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n,
            match i18n.language() {
                Language::Es => "LaunchDaemon presente",
                Language::En => "LaunchDaemon found",
            },
            plist_path.display().to_string(),
            None,
        )
    } else {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            match i18n.language() {
                Language::Es => "LaunchDaemon no instalado",
                Language::En => "LaunchDaemon not installed",
            },
            plist_path.display().to_string(),
            Some(match i18n.language() {
                Language::Es => "Sin daemon, PARAMO solo actuará cuando lo ejecutes manualmente.",
                Language::En => "Without the daemon, PARAMO only runs when launched manually.",
            }),
        )
    });

    diagnostics.push(if config.sites.list.is_empty() {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            match i18n.language() {
                Language::Es => "No hay sitios configurados",
                Language::En => "No sites configured",
            },
            match i18n.language() {
                Language::Es => "La lista de sitios está vacía.",
                Language::En => "The site list is empty.",
            }
            .to_string(),
            Some(match i18n.language() {
                Language::Es => "Añade sitios con `paramo site add dominio.com`.",
                Language::En => "Add sites with `paramo site add domain.com`.",
            }),
        )
    } else {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n,
            match i18n.language() {
                Language::Es => "Lista de sitios cargada",
                Language::En => "Site list loaded",
            },
            format!(
                "{}: {}",
                i18n.configured_sites_label(),
                config.sites.list.len()
            ),
            None,
        )
    });

    let hosts_content = match hosts::read_hosts(&config.hosts.file) {
        Ok(content) => content,
        Err(error) => {
            diagnostics.push(diagnostic(
                DiagnosticLevel::Error,
                i18n,
                match i18n.language() {
                    Language::Es => "No se pudo leer el archivo hosts",
                    Language::En => "Could not read the hosts file",
                },
                error.to_string(),
                Some(match i18n.language() {
                    Language::Es => {
                        "Comprueba que el archivo exista y que PARAMO tenga permisos para leerlo."
                    }
                    Language::En => "Check that the file exists and PARAMO can read it.",
                }),
            ));
            return Ok(diagnostics);
        }
    };
    let marker_count = hosts::count_markers(&hosts_content, &config.hosts.marker);
    diagnostics.push(if marker_count > 2 {
        diagnostic(
            DiagnosticLevel::Warn,
            i18n,
            match i18n.language() {
                Language::Es => "Secciones duplicadas en hosts",
                Language::En => "Duplicate hosts sections detected",
            },
            format!("{} markers", marker_count),
            Some(match i18n.language() {
                Language::Es => {
                    "Ejecuta `sudo paramo block` o `sudo paramo unblock` para normalizar el bloque."
                }
                Language::En => {
                    "Run `sudo paramo block` or `sudo paramo unblock` to normalize the block."
                }
            }),
        )
    } else {
        diagnostic(
            DiagnosticLevel::Ok,
            i18n,
            match i18n.language() {
                Language::Es => "Archivo hosts consistente",
                Language::En => "Hosts file looks consistent",
            },
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
                match i18n.language() {
                    Language::Es => "El bloque activo no coincide con la configuración",
                    Language::En => "Active hosts block does not match the configuration",
                },
                match i18n.language() {
                    Language::Es => {
                        "Hay una discrepancia entre `/etc/hosts` y la lista de sitios configurados."
                    }
                    Language::En => {
                        "There is a mismatch between `/etc/hosts` and the configured site list."
                    }
                }
                .to_string(),
                Some(match i18n.language() {
                    Language::Es => "Ejecuta `sudo paramo run` para resincornizar el bloqueo.",
                    Language::En => "Run `sudo paramo run` to resync blocking.",
                }),
            )
        } else {
            diagnostic(
                DiagnosticLevel::Ok,
                i18n,
                match i18n.language() {
                    Language::Es => "Bloque hosts sincronizado",
                    Language::En => "Hosts block is synchronized",
                },
                match i18n.language() {
                    Language::Es => {
                        "La sección gestionada por PARAMO coincide con la configuración."
                    }
                    Language::En => "The PARAMO-managed section matches the configuration.",
                }
                .to_string(),
                None,
            )
        },
    );

    diagnostics.push(diagnostic(
        DiagnosticLevel::Warn,
        i18n,
        match i18n.language() {
            Language::Es => "Recuerda revisar DNS over HTTPS en navegadores",
            Language::En => "Remember to review DNS over HTTPS in browsers",
        },
        match i18n.language() {
            Language::Es => {
                "Firefox y otros navegadores pueden ignorar `/etc/hosts` si DoH está activo."
            }
            Language::En => {
                "Firefox and other browsers can ignore `/etc/hosts` when DoH is enabled."
            }
        }
        .to_string(),
        Some(match i18n.language() {
            Language::Es => "Si una web sigue entrando, desactiva DoH en el navegador.",
            Language::En => "If a site still opens, disable DoH in the browser.",
        }),
    ));

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
