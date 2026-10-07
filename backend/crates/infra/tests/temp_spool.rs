#![allow(clippy::unwrap_used)]

use grove_infra::temp_spool;
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("grove-spool-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn request_spool_preserves_bytes_and_abort_is_idempotent() {
    let dir = Scratch::new();
    let (path, mut file) = temp_spool::begin_write(&dir.0, "request").await.unwrap();
    assert_eq!(path, dir.0.join(".fg-tmp-request"));
    file.write_all(b"payload").await.unwrap();
    file.flush().await.unwrap();
    drop(file);
    assert_eq!(tokio::fs::read(&path).await.unwrap(), b"payload");
    temp_spool::abort_write(&path).await;
    temp_spool::abort_write(&path).await;
    assert!(!path.exists());
}

#[tokio::test]
async fn sweep_removes_only_old_request_files() {
    let dir = Scratch::new();
    let old = dir.0.join(".fg-tmp-old");
    let unrelated = dir.0.join("unrelated");
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    for path in [&old, &unrelated] {
        let file = std::fs::File::create(path).unwrap();
        file.set_times(std::fs::FileTimes::new().set_modified(now - Duration::from_secs(3600)))
            .unwrap();
    }
    let (fresh, file) = temp_spool::begin_write(&dir.0, "fresh").await.unwrap();
    file.into_std()
        .await
        .set_times(std::fs::FileTimes::new().set_modified(now))
        .unwrap();
    let nested = dir.0.join(".fg-tmp-directory");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("keep"), b"keep").unwrap();

    assert_eq!(
        temp_spool::sweep_stale_temps_at(&dir.0, Duration::from_secs(60), now)
            .await
            .unwrap(),
        1
    );
    assert!(!old.exists());
    assert!(fresh.exists());
    assert!(unrelated.exists());
    assert!(nested.join("keep").exists());
    assert_eq!(
        temp_spool::sweep_stale_temps_at(&dir.0, Duration::from_secs(60), now)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn sweep_uses_injected_time_and_a_strict_age_boundary() {
    let dir = Scratch::new();
    let path = dir.0.join(".fg-tmp-boundary");
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    let file = std::fs::File::create(&path).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(at))
        .unwrap();
    drop(file);
    for now in [at - Duration::from_secs(1), at + Duration::from_secs(60)] {
        assert_eq!(
            temp_spool::sweep_stale_temps_at(&dir.0, Duration::from_secs(60), now)
                .await
                .unwrap(),
            0
        );
        assert!(path.exists());
    }
    assert_eq!(
        temp_spool::sweep_stale_temps_at(
            &dir.0,
            Duration::from_secs(60),
            at + Duration::from_secs(60) + Duration::from_nanos(1)
        )
        .await
        .unwrap(),
        1
    );
    assert!(!path.exists());
}
