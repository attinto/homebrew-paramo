use crate::blocker;
use crate::config::SystemConfig;
use crate::paths;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use tracing::{error, info};

fn handle_connection(mut stream: UnixStream) {
    let reader = BufReader::new(stream.try_clone().expect("clone ipc stream"));
    if let Some(Ok(line)) = reader.lines().next() {
        let config = SystemConfig::load().unwrap_or_default();
        let response = match line.trim() {
            "block" => match blocker::block_now(&config) {
                Ok(()) => {
                    info!("IPC: block applied");
                    "ok".to_string()
                }
                Err(e) => format!("err: {e}"),
            },
            "unblock" => match blocker::unblock_now(&config) {
                Ok(()) => {
                    info!("IPC: unblock applied");
                    "ok".to_string()
                }
                Err(e) => format!("err: {e}"),
            },
            "sync" => match blocker::run(&config) {
                Ok(_) => {
                    info!("IPC: sync completed");
                    "ok".to_string()
                }
                Err(e) => format!("err: {e}"),
            },
            other => format!("err: unknown command: {other}"),
        };
        let _ = writeln!(stream, "{response}");
    }
}

pub fn listen() {
    let socket_path = paths::IPC_SOCKET;
    let _ = std::fs::remove_file(socket_path);

    let listener = match UnixListener::bind(socket_path) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind IPC socket at {}: {}", socket_path, e);
            return;
        }
    };

    if let Err(e) = std::fs::set_permissions(
        socket_path,
        std::fs::Permissions::from_mode(0o666),
    ) {
        error!("Failed to set IPC socket permissions: {}", e);
    }

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    std::thread::spawn(|| handle_connection(stream));
                }
                Err(e) => {
                    error!("IPC accept error: {}", e);
                }
            }
        }
    });
}

pub fn send_command(command: &str) -> Result<String, String> {
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

    Ok(response.trim().to_string())
}
