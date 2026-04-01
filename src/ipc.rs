use crate::blocker;
use crate::config::SystemConfig;
use crate::paths;
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use tracing::{error, info};

fn handle_connection(mut stream: UnixStream, allowed_uid: u32) {
    match peer_euid(&stream) {
        Ok(uid) if uid == allowed_uid || uid == 0 => {}
        Ok(uid) => {
            let _ = writeln!(stream, "Unauthorized client uid: {uid}");
            return;
        }
        Err(error) => {
            let _ = writeln!(stream, "Failed to validate client identity: {error}");
            return;
        }
    }

    let reader = BufReader::new(stream.try_clone().expect("clone ipc stream"));
    if let Some(Ok(line)) = reader.lines().next() {
        let config = SystemConfig::load().unwrap_or_default();
        let response = match line.trim() {
            "block" => match blocker::block_now(&config) {
                Ok(()) => {
                    info!("IPC: block applied");
                    "ok".to_string()
                }
                Err(e) => e.to_string(),
            },
            "unblock" => match blocker::unblock_now(&config) {
                Ok(()) => {
                    info!("IPC: unblock applied");
                    "ok".to_string()
                }
                Err(e) => e.to_string(),
            },
            "sync" => match blocker::run(&config) {
                Ok(_) => {
                    info!("IPC: sync completed");
                    "ok".to_string()
                }
                Err(e) => e.to_string(),
            },
            other => format!("Unknown command: {other}"),
        };
        let _ = writeln!(stream, "{response}");
    }
}

pub fn listen() -> Result<()> {
    let (allowed_uid, allowed_gid) = socket_owner_ids()?;
    let socket_path = paths::IPC_SOCKET;
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("failed to bind IPC socket at {}", socket_path))?;

    set_socket_owner(socket_path, allowed_uid, allowed_gid)?;
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to secure IPC socket at {}", socket_path))?;

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    std::thread::spawn(move || handle_connection(stream, allowed_uid));
                }
                Err(e) => {
                    error!("IPC accept error: {}", e);
                }
            }
        }
    });

    Ok(())
}

pub fn send_command(command: &str) -> Result<(), String> {
    let stream = UnixStream::connect(paths::IPC_SOCKET).map_err(|_| {
        "El daemon no está activo. Ejecuta: sudo paramo install".to_string()
    })?;

    let mut writer = &stream;
    writeln!(writer, "{command}").map_err(|e| format!("Error al enviar comando: {e}"))?;

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .map_err(|e| format!("Error al leer respuesta: {e}"))?;

    let response = response.trim();
    if response == "ok" {
        Ok(())
    } else if response.is_empty() {
        Err("El daemon no devolvió ninguna respuesta.".to_string())
    } else {
        Err(response.to_string())
    }
}

fn socket_owner_ids() -> Result<(u32, u32)> {
    let candidate = [paths::system_config_file(), paths::system_config_dir()]
        .into_iter()
        .find(|path| path.exists());

    let Some(path) = candidate else {
        return Ok((unsafe { libc::geteuid() }, unsafe { libc::getegid() }));
    };

    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("failed to inspect socket owner source {}", path.display()))?;
    Ok((metadata.uid(), metadata.gid()))
}

fn set_socket_owner(path: &str, uid: u32, gid: u32) -> Result<()> {
    let path = std::ffi::CString::new(path).context("IPC socket path contains a null byte")?;
    let status = unsafe { libc::chown(path.as_ptr(), uid, gid) };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error()).context("failed to set IPC socket owner")
    }
}

fn peer_euid(stream: &UnixStream) -> std::io::Result<u32> {
    let mut euid: libc::uid_t = 0;
    let mut egid: libc::gid_t = 0;
    let status = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut euid, &mut egid) };
    if status == 0 {
        Ok(euid)
    } else {
        Err(std::io::Error::last_os_error())
    }
}
