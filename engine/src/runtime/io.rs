use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tokio::{io::AsyncWriteExt, sync::mpsc, task::JoinHandle};

pub struct AsyncLineWriter {
    pub sender: mpsc::Sender<String>,
    pub task: JoinHandle<Result<u64, io::Error>>,
    pub written: Arc<AtomicU64>,
    pub dropped: Arc<AtomicU64>,
}

pub async fn spawn_line_writer(
    path: Option<PathBuf>,
    channel_capacity: usize,
    buffer_capacity: usize,
    flush_every: u32,
) -> AsyncLineWriter {
    let (sender, mut receiver) = mpsc::channel::<String>(channel_capacity.max(1));
    let written = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let written_count = Arc::clone(&written);
    let task = tokio::spawn(async move {
        let Some(path) = path else {
            while receiver.recv().await.is_some() {
                written_count.fetch_add(1, Ordering::Relaxed);
            }
            return Ok(written_count.load(Ordering::Relaxed));
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = tokio::fs::File::create(path).await?;
        let mut writer = tokio::io::BufWriter::with_capacity(buffer_capacity.max(1), file);
        let mut pending = 0_u32;
        while let Some(line) = receiver.recv().await {
            writer.write_all(line.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            written_count.fetch_add(1, Ordering::Relaxed);
            pending += 1;
            if pending >= flush_every.max(1) {
                writer.flush().await?;
                pending = 0;
            }
        }
        writer.flush().await?;
        Ok(written_count.load(Ordering::Relaxed))
    });
    AsyncLineWriter {
        sender,
        task,
        written,
        dropped,
    }
}

pub async fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), io::Error> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }
    // A unique temporary name prevents concurrent snapshots from colliding.
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let temporary = path.with_extension(format!("json.tmp.{}.{}", std::process::id(), nonce));
    tokio::fs::write(&temporary, bytes).await?;
    let mut last_error = None;
    for attempt in 0..8_u32 {
        match tokio::fs::rename(&temporary, path).await {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = tokio::fs::remove_file(path).await;
                last_error = Some(error);
                if attempt < 7 {
                    tokio::time::sleep(Duration::from_millis(2_u64 << attempt)).await;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                last_error = Some(error);
                if attempt < 7 {
                    tokio::time::sleep(Duration::from_millis(2_u64 << attempt)).await;
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, "atomic snapshot rename failed")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writer_counts_drained_lines_without_a_path() {
        let writer = spawn_line_writer(None, 2, 32, 1).await;
        writer.sender.send("one".to_owned()).await.unwrap();
        drop(writer.sender);
        assert_eq!(writer.task.await.unwrap().unwrap(), 1);
    }
}
