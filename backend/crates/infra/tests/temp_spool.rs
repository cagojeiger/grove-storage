#![allow(clippy::unwrap_used)]

use grove_infra::temp_spool;
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("grove-spool-{}", uuid::Uuid::new_v4()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path).unwrap();
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
async fn request_spool_never_truncates_an_existing_file() {
    let dir = Scratch::new();
    let (path, file) = temp_spool::begin_write(&dir.0, "exclusive").await.unwrap();
    drop(file);
    std::fs::write(&path, b"keep").unwrap();
    assert!(temp_spool::begin_write(&dir.0, "exclusive").await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"keep");
}

#[cfg(unix)]
#[tokio::test]
async fn private_permissions_and_symlink_rejection() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = Scratch::new();
    let root = dir.0.join("private");
    let (path, file) = temp_spool::begin_write(&root, "file").await.unwrap();
    drop(file);
    assert_eq!(
        std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let target = dir.0.join("target");
    std::fs::write(&target, b"keep").unwrap();
    symlink(&target, root.join(".fg-tmp-link")).unwrap();
    assert!(temp_spool::begin_write(&root, "link").await.is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"keep");
    let linked_root = dir.0.join("linked-root");
    symlink(&root, &linked_root).unwrap();
    assert!(temp_spool::begin_write(&linked_root, "new").await.is_err());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(temp_spool::begin_write(&root, "new").await.is_err());
}

#[tokio::test]
async fn cancelled_request_unlinks_its_private_spool() {
    let dir = Scratch::new();
    let root = dir.0.clone();
    let (ready, started) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let (path, file) = temp_spool::begin_write(&root, "cancel").await.unwrap();
        let _cleanup = temp_spool::Cleanup(path.clone());
        let _file = file;
        ready.send(path).unwrap();
        std::future::pending::<()>().await;
    });
    let path = started.await.unwrap();
    assert!(path.exists());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
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
