use crate::paths;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// Evita loguear el mismo dominio+proto más de una vez cada 5 segundos
// (los browsers reintentan la conexión varias veces de golpe)
static DEBOUNCE: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct WitnessEntry {
    pub timestamp: DateTime<Local>,
    pub domain: String,
    pub proto: String,
}

/// Arranca los listeners en background. Llamar una sola vez desde el daemon.
/// Si el puerto ya está en uso, falla silenciosamente.
pub fn start() {
    std::thread::Builder::new()
        .name("witness-http".into())
        .spawn(|| listen(80, "http"))
        .ok();
    std::thread::Builder::new()
        .name("witness-https".into())
        .spawn(|| listen(443, "https"))
        .ok();
}

fn listen(port: u16, proto: &'static str) {
    let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
        return;
    };
    for mut stream in listener.incoming().flatten() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
        let mut buf = [0u8; 512];
        let Ok(n) = stream.read(&mut buf) else {
            continue;
        };
        if n == 0 {
            continue;
        }
        let domain = if port == 443 {
            extract_sni(&buf[..n])
        } else {
            extract_http_host(&buf[..n])
        };
        if let Some(domain) = domain {
            if debounce_allow(&domain, proto) {
                log_attempt(&domain, proto);
            }
        }
        // stream se cierra al salir del bloque — la conexión queda rechazada
    }
}

// --- Extracción de dominio ---

fn extract_http_host(buf: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(buf).ok()?;
    for line in text.lines() {
        if line.len() >= 5 && line[..5].eq_ignore_ascii_case("Host:") {
            let host = line[5..].trim().to_ascii_lowercase();
            // quitar puerto si viene ":80"
            let host = host.split(':').next().unwrap_or("").trim().to_string();
            if !host.is_empty() {
                return Some(host);
            }
        }
    }
    None
}

/// Extrae el hostname de la extensión SNI de un TLS ClientHello.
///
/// Formato del buffer (simplificado):
///   [0]      = 0x16  (TLS record type: Handshake)
///   [1..2]   = versión legacy
///   [3..4]   = longitud del record
///   [5]      = 0x01  (HandshakeType: ClientHello)
///   [6..8]   = longitud del handshake (3 bytes)
///   [9..10]  = client version
///   [11..42] = random (32 bytes)
///   [43]     = session_id length  ← pos de inicio
///   ...cipher suites, compression, extensions...
///   extension type 0x0000 = SNI → list_len(2) + name_type(1) + name_len(2) + name
fn extract_sni(buf: &[u8]) -> Option<String> {
    if buf.len() < 44 {
        return None;
    }
    if buf[0] != 0x16 || buf[5] != 0x01 {
        return None;
    }

    let mut pos: usize = 43; // byte del campo session_id_length

    // session_id
    let sid_len = buf[pos] as usize;
    pos = pos.checked_add(1 + sid_len)?;

    // cipher_suites
    if pos + 2 > buf.len() {
        return None;
    }
    let cs_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos = pos.checked_add(2 + cs_len)?;

    // compression_methods
    if pos >= buf.len() {
        return None;
    }
    let cm_len = buf[pos] as usize;
    pos = pos.checked_add(1 + cm_len)?;

    // extensions: saltamos la longitud total (2 bytes) y recorremos una a una
    if pos + 2 > buf.len() {
        return None;
    }
    pos += 2;

    while pos + 4 <= buf.len() {
        let ext_type = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let ext_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;

        if ext_type == 0x0000 {
            // SNI: list_len(2) + name_type(1) + name_len(2) + name
            if pos + 5 > buf.len() {
                return None;
            }
            let name_len = u16::from_be_bytes([buf[pos + 3], buf[pos + 4]]) as usize;
            let name_start = pos + 5;
            if name_start + name_len > buf.len() {
                return None;
            }
            return String::from_utf8(buf[name_start..name_start + name_len].to_vec()).ok();
        }

        pos = pos.checked_add(ext_len)?;
    }
    None
}

// --- Deduplicación y escritura ---

fn debounce_allow(domain: &str, proto: &str) -> bool {
    let key = format!("{}|{}", domain, proto);
    let map = DEBOUNCE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if guard
        .get(&key)
        .map_or(false, |t| t.elapsed() < Duration::from_secs(5))
    {
        return false;
    }
    guard.insert(key, now);
    true
}

fn log_attempt(domain: &str, proto: &str) {
    let path = paths::user_witness_file();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{}|{}|{}", Local::now().to_rfc3339(), domain, proto);
    }
}

// --- Lectura del log ---

pub fn load_today() -> Vec<WitnessEntry> {
    let today = Local::now().date_naive();
    load_all()
        .into_iter()
        .filter(|e| e.timestamp.date_naive() == today)
        .collect()
}

pub fn load_all() -> Vec<WitnessEntry> {
    let path = paths::user_witness_file();
    let Ok(file) = std::fs::File::open(path) else {
        return vec![];
    };
    BufReader::new(file)
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            let mut parts = line.splitn(3, '|');
            let ts = parts.next()?;
            let domain = parts.next()?;
            let proto = parts.next()?;
            Some(WitnessEntry {
                timestamp: DateTime::parse_from_rfc3339(ts)
                    .ok()?
                    .with_timezone(&Local),
                domain: domain.to_string(),
                proto: proto.trim().to_string(),
            })
        })
        .collect()
}

// --- Formateo del reporte ---

pub fn format_today_report() -> String {
    let entries = load_today();
    let date_str = Local::now().format("%d/%m/%Y").to_string();

    if entries.is_empty() {
        return format!(
            "\n  El Testigo · {}\n\n  Sin intentos registrados hoy. ¡Sigue así!\n\n",
            date_str
        );
    }

    let mut out = format!("\n  El Testigo · {}\n\n", date_str);

    for entry in &entries {
        out.push_str(&format!(
            "  {}  {:<30}  {}\n",
            entry.timestamp.format("%H:%M"),
            entry.domain,
            entry.proto,
        ));
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    for e in &entries {
        *counts.entry(e.domain.clone()).or_insert(0) += 1;
    }
    let top = counts.iter().max_by_key(|(_, c)| *c);

    out.push_str("\n  ──────────────────────────────────────\n");
    out.push_str(&format!("  Total hoy:        {} intentos\n", entries.len()));
    out.push_str(&format!("  Dominios únicos:  {}\n", counts.len()));
    if let Some((domain, count)) = top {
        out.push_str(&format!("  Más intentado:    {} ({}x)\n", domain, count));
    }
    out.push('\n');
    out
}
