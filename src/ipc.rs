use crate::blocker;
use crate::config::SystemConfig;
use crate::paths;
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "macos")]
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc;
use tracing::{error, info, warn};

// Número máximo de conexiones simultáneas al daemon.
// Para un bloqueador de sitios local esto es más que suficiente.
const MAX_WORKERS: usize = 4;

fn handle_connection(mut stream: UnixStream, allowed_uid: u32) {
    match peer_euid(&stream) {
        Ok(uid) if uid == allowed_uid || uid == 0 => {}
        Ok(uid) => {
            let _ = writeln!(stream, "Unauthorized client uid: {uid}");
            return;
        }
        Err(e) => {
            let _ = writeln!(stream, "Failed to validate client identity: {e}");
            return;
        }
    }

    let reader = match stream.try_clone() {
        Ok(cloned) => BufReader::new(cloned),
        Err(e) => {
            error!("IPC: failed to clone stream: {}", e);
            let _ = writeln!(stream, "Internal error: could not read command");
            return;
        }
    };

    if let Some(Ok(line)) = reader.lines().next() {
        let config = SystemConfig::load().unwrap_or_default();
        let response = match line.trim() {
            "block" => match blocker::block_now(&config) {
                Ok(()) => {
                    info!("IPC: block applied");
                    "ok".to_string()
                }
                Err(e) => {
                    error!("IPC: block failed: {}", e);
                    e.to_string()
                }
            },
            "unblock" => match blocker::unblock_now(&config) {
                Ok(()) => {
                    info!("IPC: unblock applied");
                    "ok".to_string()
                }
                Err(e) => {
                    error!("IPC: unblock failed: {}", e);
                    e.to_string()
                }
            },
            "sync" => match blocker::run(&config) {
                Ok(_) => {
                    info!("IPC: sync completed");
                    "ok".to_string()
                }
                Err(e) => {
                    error!("IPC: sync failed: {}", e);
                    e.to_string()
                }
            },
            other => {
                warn!("IPC: unknown command '{}'", other);
                format!("Unknown command: {other}")
            }
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

    // Canal de capacidad limitada: si está lleno, la conexión entrante se descarta
    // en lugar de crear un thread sin límite.
    let (tx, rx) = mpsc::sync_channel::<UnixStream>(MAX_WORKERS * 2);

    // Receptor compartido entre los workers del pool
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));

    for _ in 0..MAX_WORKERS {
        let rx = std::sync::Arc::clone(&rx);
        std::thread::spawn(move || loop {
            let stream = match rx.lock() {
                Ok(guard) => guard.recv(),
                Err(_) => break,
            };
            match stream {
                Ok(stream) => handle_connection(stream, allowed_uid),
                Err(_) => break, // canal cerrado, el worker termina limpiamente
            }
        });
    }

    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => {
                    if tx.try_send(stream).is_err() {
                        warn!("IPC: connection queue full, dropping connection");
                    }
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
    let stream = UnixStream::connect(paths::IPC_SOCKET)
        .map_err(|_| "El daemon no está activo. Ejecuta: sudo paramo install".to_string())?;

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

// getpeereid es exclusivo de macOS/BSD. En Linux se usa SO_PEERCRED.
// En producción esto solo corre en macOS (app macOS-only).
#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
fn peer_euid(_stream: &UnixStream) -> std::io::Result<u32> {
    // Fallback para builds de desarrollo en Linux.
    // En producción paramo solo corre en macOS.
    Ok(unsafe { libc::getuid() })
}
