use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PLIST_TEMPLATE: &str = include_str!("../launchd/com.paramo.blocker.plist");
const BINARY_PLACEHOLDER: &str = "__PARAMO_BINARY__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStatus {
    pub loaded: bool,
    pub disabled: Option<bool>,
    pub pid: Option<String>,
    pub last_exit_status: Option<String>,
    pub program: Option<PathBuf>,
}

pub fn render_plist(binary_path: &Path) -> String {
    PLIST_TEMPLATE.replace(
        BINARY_PLACEHOLDER,
        &escape_xml(&binary_path.display().to_string()),
    )
}

pub fn query_service(label: &str) -> Result<ServiceStatus> {
    let disabled = service_disabled(label)?;
    let target = format!("system/{label}");
    let output = command_output("launchctl", &["print", &target])?;

    if output.status.success() {
        let stdout = decode(&output.stdout);
        let parsed = parse_launchctl_print(&stdout);
        return Ok(ServiceStatus {
            loaded: true,
            disabled,
            pid: parsed.pid,
            last_exit_status: parsed.last_exit_status,
            program: parsed.program,
        });
    }

    let combined = combined_output(&output);
    if is_missing_service_message(&combined) {
        return Ok(ServiceStatus {
            loaded: false,
            disabled,
            pid: None,
            last_exit_status: None,
            program: None,
        });
    }

    bail!(command_failure_message(
        "launchctl",
        &["print", &target],
        &output
    ));
}

pub fn bootout_service(label: &str) -> Result<bool> {
    let status = query_service(label)?;
    if !status.loaded {
        return Ok(false);
    }

    let target = format!("system/{label}");
    let output = command_output("launchctl", &["bootout", &target])?;
    if output.status.success() {
        return Ok(true);
    }

    let status_after = query_service(label)?;
    if !status_after.loaded {
        return Ok(true);
    }

    bail!(command_failure_message(
        "launchctl",
        &["bootout", &target],
        &output
    ));
}

pub fn bootstrap_target(target: &str) -> Result<()> {
    run_command("launchctl", &["bootstrap", "system", target]).map(|_| ())
}

pub fn kickstart_service(label: &str) -> Result<()> {
    let target = format!("system/{label}");
    run_command("launchctl", &["kickstart", "-k", &target]).map(|_| ())
}

pub fn service_disabled(label: &str) -> Result<Option<bool>> {
    let output = command_output("launchctl", &["print-disabled", "system"])?;
    if !output.status.success() {
        bail!(command_failure_message(
            "launchctl",
            &["print-disabled", "system"],
            &output
        ));
    }

    let stdout = decode(&output.stdout);
    let needle = format!("\"{label}\" => ");
    let line = stdout.lines().find(|line| line.contains(&needle));
    let Some(line) = line else {
        return Ok(None);
    };

    let state = line.split_once(&needle).map(|(_, value)| value.trim());
    Ok(match state {
        Some("disabled") | Some("true") => Some(true),
        Some("enabled") | Some("false") => Some(false),
        _ => None,
    })
}

pub fn plist_value(content: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{key}</key>");
    let after_key = content.split_once(&key_tag)?.1;
    let after_string = after_key.split_once("<string>")?.1;
    let value = after_string.split_once("</string>")?.0;
    Some(unescape_xml(value))
}

pub fn plist_integer(content: &str, key: &str) -> Option<u32> {
    let key_tag = format!("<key>{key}</key>");
    let after_key = content.split_once(&key_tag)?.1;
    let after_integer = after_key.split_once("<integer>")?.1;
    after_integer
        .split_once("</integer>")?
        .0
        .trim()
        .parse()
        .ok()
}

pub fn plist_program_arguments(content: &str) -> Vec<String> {
    let key_tag = "<key>ProgramArguments</key>";
    let Some(after_key) = content.split_once(key_tag).map(|(_, rest)| rest) else {
        return Vec::new();
    };
    let Some(array) = after_key
        .split_once("<array>")
        .and_then(|(_, rest)| rest.split_once("</array>").map(|(array, _)| array))
    else {
        return Vec::new();
    };

    array
        .split("<string>")
        .skip(1)
        .filter_map(|segment| {
            segment
                .split_once("</string>")
                .map(|(value, _)| unescape_xml(value))
        })
        .collect()
}

fn run_command(program: &str, args: &[&str]) -> Result<Output> {
    let output = command_output(program, args)?;
    if output.status.success() {
        Ok(output)
    } else {
        bail!(command_failure_message(program, args, &output));
    }
}

fn command_output(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run `{}`", format_command(program, args)))
}

fn command_failure_message(program: &str, args: &[&str], output: &Output) -> String {
    let stdout = decode(&output.stdout);
    let stderr = decode(&output.stderr);
    let mut lines = vec![format!(
        "`{}` exited with status {}",
        format_command(program, args),
        output.status
    )];

    if !stdout.is_empty() {
        lines.push(format!("stdout: {stdout}"));
    }
    if !stderr.is_empty() {
        lines.push(format!("stderr: {stderr}"));
    }

    lines.join(" | ")
}

fn format_command(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

fn combined_output(output: &Output) -> String {
    let stdout = decode(&output.stdout);
    let stderr = decode(&output.stderr);

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn is_missing_service_message(text: &str) -> bool {
    text.contains("Could not find service")
        || text.contains("service not found")
        || text.contains("No such process")
}

#[derive(Debug, Default)]
struct ParsedLaunchctlPrint {
    pid: Option<String>,
    last_exit_status: Option<String>,
    program: Option<PathBuf>,
}

fn parse_launchctl_print(output: &str) -> ParsedLaunchctlPrint {
    let mut parsed = ParsedLaunchctlPrint::default();

    for line in output.lines().map(str::trim) {
        if parsed.pid.is_none() {
            parsed.pid = extract_assignment(line, &["pid", "PID"]);
        }

        if parsed.last_exit_status.is_none() {
            parsed.last_exit_status = extract_assignment(
                line,
                &["last exit code", "last exit status", "LastExitStatus"],
            );
        }

        if parsed.program.is_none() {
            let program = extract_assignment(line, &["program", "Program", "path", "Path"]);
            parsed.program = program
                .filter(|value| value.starts_with('/'))
                .map(PathBuf::from);
        }
    }

    parsed
}

fn extract_assignment(line: &str, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let needle = format!("{key} = ");
        line.strip_prefix(&needle)
            .map(|value| value.trim().to_string())
    })
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_render_plist_replaces_template_values() {
        let plist = render_plist(Path::new("/usr/local/bin/paramo"));
        assert!(plist.contains("<string>/usr/local/bin/paramo</string>"));
        assert!(!plist.contains(BINARY_PLACEHOLDER));
    }

    #[test]
    fn test_program_arguments_are_extracted() {
        let plist = render_plist(Path::new("/usr/local/bin/paramo"));
        assert_eq!(
            plist_program_arguments(&plist),
            vec!["/usr/local/bin/paramo".to_string(), "run".to_string()]
        );
    }

    #[test]
    fn test_parse_launchctl_print_extracts_common_fields() {
        let output = r#"
com.paramo.blocker = {
    pid = 123
    program = /usr/local/bin/paramo
    last exit code = 0
}
"#;

        let parsed = parse_launchctl_print(output);
        assert_eq!(parsed.pid.as_deref(), Some("123"));
        assert_eq!(parsed.last_exit_status.as_deref(), Some("0"));
        assert_eq!(parsed.program, Some(PathBuf::from("/usr/local/bin/paramo")));
    }
}
