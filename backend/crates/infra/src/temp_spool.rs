//! Disposable local transfer buffers, not an object storage backend.

use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn begin_write(root: &Path, temp_name: &str) -> anyhow::Result<(PathBuf, fs::File)> {
    let temp = root.join(format!(".fg-tmp-{temp_name}"));
    let file = fs::File::create(&temp).await?;
    Ok((temp, file))
}

pub async fn abort_write(temp: &Path) {
    let _ = fs::remove_file(temp).await;
}

/// Reap old request spools left by interrupted processes using the caller's
/// retention window. Live request spools must remain younger than that window.
pub async fn sweep_stale_temps(dir: &Path, max_age: std::time::Duration) -> anyhow::Result<u32> {
    use filegate_core::time::Clock;
    sweep_stale_temps_at(dir, max_age, filegate_core::time::SystemClock.now().into()).await
}

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
