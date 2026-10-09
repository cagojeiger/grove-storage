//! Disposable local transfer buffers, not an object storage backend.

use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn prepare_root(root: &Path) -> anyhow::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    match builder.create(root).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(root).await?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.is_symlink(),
        "spool root must be a directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        anyhow::ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "spool root must be owner-only"
        );
    }
    Ok(())
}

pub async fn begin_write(root: &Path, temp_name: &str) -> anyhow::Result<(PathBuf, fs::File)> {
    prepare_root(root).await?;
    let temp = root.join(format!(".fg-tmp-{temp_name}"));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(&temp).await?;
    Ok((temp, file))
}

/// Unlink on cancellation as well as normal return; create this before the writer.
pub struct Cleanup(pub PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub async fn abort_write(temp: &Path) {
    let _ = fs::remove_file(temp).await;
}

/// Reap old request spools left by interrupted processes using the caller's
/// retention window. Live request spools must remain younger than that window.
pub async fn sweep_stale_temps_at(
    dir: &Path,
    max_age: std::time::Duration,
    now: std::time::SystemTime,
) -> anyhow::Result<u32> {
    let mut entries = fs::read_dir(dir).await?;
    let mut removed = 0u32;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(".fg-tmp-") {
            continue;
        }
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        let stale = meta
            .modified()
            .ok()
            .and_then(|mtime| now.duration_since(mtime).ok())
            .is_some_and(|age| age > max_age);
        if stale && fs::remove_file(entry.path()).await.is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}
