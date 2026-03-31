use std::collections::BTreeSet;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HostsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type HostsResult<T> = Result<T, HostsError>;

pub fn read_hosts(path: &Path) -> HostsResult<String> {
    Ok(std::fs::read_to_string(path)?)
}

pub fn write_hosts_atomic(path: &Path, content: &str) -> HostsResult<()> {
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

pub fn is_blocked(content: &str, marker: &str) -> bool {
    content.contains(marker)
}

pub fn build_block_section(marker: &str, domains: &[String], redirect_ips: &[String]) -> String {
    let mut lines = vec![marker.to_string()];

    for domain in expand_domains(domains) {
        for ip in redirect_ips {
            lines.push(format!("{} {}", ip, domain));
        }
    }

    lines.push(marker.to_string());
    lines.join("\n")
}

pub fn get_existing_block_section(content: &str, marker: &str) -> HostsResult<Option<String>> {
    let mut lines = Vec::new();
    let mut inside_block = false;

    for line in content.lines() {
        if line == marker {
            lines.push(line.to_string());

            if inside_block {
                return Ok(Some(lines.join("\n")));
            }

            inside_block = true;
            continue;
        }

        if inside_block {
            lines.push(line.to_string());
        }
    }

    Ok(None)
}

pub fn has_expected_block(
    content: &str,
    marker: &str,
    domains: &[String],
    redirect_ips: &[String],
) -> HostsResult<bool> {
    let existing_block = get_existing_block_section(content, marker)?;
    let expected_block = build_block_section(marker, domains, redirect_ips);

    Ok(existing_block.map(|b| b == expected_block).unwrap_or(false))
}

pub fn add_block(content: &str, block_section: &str) -> String {
    format!("{}\n\n{}\n", content.trim_end(), block_section)
}

pub fn remove_block(content: &str, marker: &str) -> HostsResult<String> {
    let mut kept_lines = Vec::new();
    let mut inside_block = false;

    for line in content.lines() {
        if line == marker {
            inside_block = !inside_block;
            continue;
        }

        if !inside_block {
            kept_lines.push(line);
        }
    }

    let result = kept_lines.join("\n");
    Ok(format!("{}\n", result.trim()))
}

fn expand_domains(domains: &[String]) -> Vec<String> {
    let mut expanded = BTreeSet::new();

    for domain in domains {
        for alias in domain_aliases(domain) {
            expanded.insert(alias);
        }
    }

    expanded.into_iter().collect()
}

fn domain_aliases(domain: &str) -> BTreeSet<String> {
    let normalized = domain.trim().to_lowercase();
    let mut aliases = BTreeSet::new();

    if normalized.is_empty() {
        return aliases;
    }

    aliases.insert(normalized.clone());

    if normalized.contains('.') {
        if let Some(stripped) = normalized.strip_prefix("www.") {
            aliases.insert(stripped.to_string());
        } else {
            aliases.insert(format!("www.{}", normalized));
        }
    }

    if is_youtube_domain(&normalized) {
        for alias in [
            "youtube.com",
            "www.youtube.com",
            "m.youtube.com",
            "music.youtube.com",
            "studio.youtube.com",
            "kids.youtube.com",
            "youtu.be",
            "youtube-nocookie.com",
            "www.youtube-nocookie.com",
        ] {
            aliases.insert(alias.to_string());
        }
    }

    aliases
}

fn is_youtube_domain(domain: &str) -> bool {
    matches!(domain, "youtube.com" | "youtu.be" | "youtube-nocookie.com")
        || domain.ends_with(".youtube.com")
        || domain.ends_with(".youtu.be")
        || domain.ends_with(".youtube-nocookie.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_block_section() {
        let marker = "# --- TEST ---";
        let domains = vec!["example.com".to_string(), "test.com".to_string()];
        let ips = vec!["127.0.0.1".to_string()];

        let result = build_block_section(marker, &domains, &ips);
        assert!(result.contains("example.com"));
        assert!(result.contains("test.com"));
        assert!(result.contains("www.example.com"));
        assert!(result.contains("127.0.0.1"));
        assert!(result.starts_with(marker));
        assert!(result.ends_with(marker));
    }

    #[test]
    fn test_add_and_remove_block() {
        let marker = "# --- TEST ---";
        let block = "# --- TEST ---\n127.0.0.1 example.com\n# --- TEST ---";
        let content = "# Original hosts content\n";

        let with_block = add_block(content, block);
        assert!(with_block.contains("# Original hosts content"));
        assert!(with_block.contains("example.com"));

        let without_block = remove_block(&with_block, marker).unwrap();
        assert!(!without_block.contains("example.com"));
    }

    #[test]
    fn test_remove_block_removes_duplicate_sections() {
        let marker = "# --- TEST ---";
        let block = "# --- TEST ---\n127.0.0.1 example.com\n# --- TEST ---";
        let content = format!("header\n\n{}\n\n{}\n", block, block);

        let without_block = remove_block(&content, marker).unwrap();
        assert_eq!(without_block, "header\n");
    }

    #[test]
    fn test_is_blocked() {
        let marker = "# --- TEST ---";
        let content_blocked = "some content\n# --- TEST ---\nblocked";
        let content_not_blocked = "some content\nno marker here";

        assert!(is_blocked(content_blocked, marker));
        assert!(!is_blocked(content_not_blocked, marker));
    }

    #[test]
    fn test_build_block_section_expands_youtube_aliases() {
        let marker = "# --- TEST ---";
        let domains = vec!["youtube.com".to_string()];
        let ips = vec!["127.0.0.1".to_string()];

        let result = build_block_section(marker, &domains, &ips);
        assert!(result.contains("127.0.0.1 youtube.com"));
        assert!(result.contains("127.0.0.1 www.youtube.com"));
        assert!(result.contains("127.0.0.1 m.youtube.com"));
        assert!(result.contains("127.0.0.1 youtu.be"));
    }
}
