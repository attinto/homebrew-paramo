use anyhow::Result;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::EnvFilter;

pub fn setup_logging(log_file: &Path, level: &str) -> Result<Option<WorkerGuard>> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let (writer, guard) = match try_file_writer(log_file) {
        Some((writer, guard)) => (writer, Some(guard)),
        None => {
            eprintln!(
                "No se pudo abrir {} para logs. Usando stderr.",
                log_file.display()
            );
            (BoxMakeWriter::new(io::stderr), None)
        }
    };

    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_env_filter(filter)
        .with_ansi(false)
        .try_init()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(guard)
}

fn try_file_writer(log_file: &Path) -> Option<(BoxMakeWriter, WorkerGuard)> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .ok()?;

    let parent_dir = log_file.parent().unwrap_or_else(|| Path::new("."));
    let file_name = log_file.file_name().unwrap_or_default();

    let file_appender = tracing_appender::rolling::daily(parent_dir, file_name);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    Some((BoxMakeWriter::new(non_blocking), guard))
}
