#![allow(clippy::unwrap_used)]
use grove_domain::*;

fn spec(bucket: &str) -> StorageSpec {
    let endpoint = Endpoint::parse("https://s3.example.com").unwrap();
    StorageSpec {
        target: S3Target::new(endpoint.clone(), "auto".into(), bucket.into(), false).unwrap(),
        public_endpoint: endpoint,
        capacity: Capacity::new(1000).unwrap(),
        force_relay: false,
    }
}

#[test]
fn every_reference_kind_independently_blocks_delete_and_retarget() {
    for references in [
        References {
            clients: 1,
            ..Default::default()
        },
        References {
            files: 1,
            ..Default::default()
        },
        References {
            uploads: 1,
            ..Default::default()
        },
        References {
            cleanup_jobs: 1,
            ..Default::default()
        },
    ] {
        assert!(!references.permits_delete());
        assert!(!references.permits_replace(&spec("old"), &spec("new")));
    }
    assert!(References::default().permits_delete());
    assert!(References::default().permits_replace(&spec("old"), &spec("new")));
}

#[test]
fn metadata_updates_do_not_change_physical_identity() {
    let current = spec("bucket");
    let mut next = current.clone();
    next.capacity = Capacity::new(0).unwrap();
    next.public_endpoint = Endpoint::parse("https://public.example.com").unwrap();
    next.force_relay = true;
    assert!(
        References {
            files: u64::MAX,
            ..Default::default()
        }
        .permits_replace(&current, &next)
    );
}

#[test]
fn routing_changes_are_conservatively_guarded() {
    let current = spec("bucket");
    for (endpoint, region, path_style) in [
        ("https://other.example.com", "auto", false),
        ("https://s3.example.com", "other", false),
        ("https://s3.example.com", "auto", true),
    ] {
        let mut next = current.clone();
        next.target = S3Target::new(
            Endpoint::parse(endpoint).unwrap(),
            region.into(),
            "bucket".into(),
            path_style,
        )
        .unwrap();
        assert!(
            !References {
                files: 1,
                ..Default::default()
            }
            .permits_replace(&current, &next)
        );
    }
}
