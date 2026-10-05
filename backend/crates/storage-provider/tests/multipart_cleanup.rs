#![allow(clippy::unwrap_used)]
use axum::{Router, body::Body, extract::State, http::Request, routing::any};
use grove_storage_provider::{Address, S3ClientCache, S3StorageSpec, s3_abort_multipart_by_key};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

struct Responses {
    pages: VecDeque<String>,
    requests: Vec<String>,
}

fn page(truncated: bool, key: Option<&str>, id: Option<&str>, uploads: &str) -> String {
    format!(
        "<ListMultipartUploadsResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><IsTruncated>{truncated}</IsTruncated>{}{}{uploads}</ListMultipartUploadsResult>",
        key.map(|key| format!("<NextKeyMarker>{key}</NextKeyMarker>"))
            .unwrap_or_default(),
        id.map(|id| format!("<NextUploadIdMarker>{id}</NextUploadIdMarker>"))
            .unwrap_or_default(),
    )
}

async fn cleanup(pages: Vec<String>) -> (anyhow::Result<()>, Vec<String>) {
    let state = Arc::new(Mutex::new(Responses {
        pages: pages.into(),
        requests: Vec::new(),
    }));
    let router = Router::new()
        .fallback(any(
            |State(state): State<Arc<Mutex<Responses>>>, req: Request<Body>| async move {
                let mut state = state.lock().unwrap();
                state
                    .requests
                    .push(format!("{} {}", req.method(), req.uri()));
                let body = if req.method() == "GET" {
                    state
                        .pages
                        .pop_front()
                        .unwrap_or_else(|| page(false, None, None, ""))
                } else {
                    String::new()
                };
                axum::http::Response::builder()
                    .status(200)
                    .body(Body::from(body))
                    .unwrap()
            },
        ))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let spec = S3StorageSpec {
        endpoint,
        public_endpoint: "http://public.invalid".into(),
        region: "local".into(),
        bucket: "objects".into(),
        force_path_style: true,
        access_key: "access".into(),
        secret_key: secrecy::SecretString::from("secret".to_owned()),
    };
    let storage = S3ClientCache::default().get("storage", &spec, Address::Internal);
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        s3_abort_multipart_by_key(&storage, "file"),
    )
    .await
    .unwrap();
    server.abort();
    let requests = state.lock().unwrap().requests.clone();
    (result, requests)
}

#[tokio::test]
async fn follows_both_markers_and_only_aborts_the_exact_key() {
    let (result, requests) = cleanup(vec![
        page(true, Some("file"), Some("first"), "<Upload><Key>file</Key><UploadId>first</UploadId></Upload><Upload><Key>file-other</Key><UploadId>unrelated</UploadId></Upload>"),
        page(true, Some("file"), Some("second"), "<Upload><Key>file</Key><UploadId>second</UploadId></Upload>"),
        page(false, None, None, ""),
    ]).await;
    result.unwrap();
    assert_eq!(requests.len(), 5);
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.starts_with("DELETE"))
            .count(),
        2
    );
    assert!(
        requests
            .iter()
            .any(|request| request.contains("uploadId=first"))
    );
    assert!(
        requests
            .iter()
            .any(|request| request.contains("uploadId=second"))
    );
    assert!(
        requests
            .iter()
            .any(|request| request.contains("key-marker=file")
                && request.contains("upload-id-marker=second"))
    );
    assert!(!requests.iter().any(|request| request.contains("unrelated")));
}

#[tokio::test]
async fn repeated_marker_fails_before_requesting_another_page() {
    let repeated = page(true, Some("file"), Some("first"), "");
    let (result, requests) = cleanup(vec![repeated.clone(), repeated]).await;
    assert!(result.unwrap_err().to_string().contains("repeated"));
    assert_eq!(requests.len(), 2);
}

#[tokio::test]
async fn marker_cycle_fails_instead_of_looping() {
    let first = page(true, Some("file"), Some("first"), "");
    let second = page(true, Some("file"), Some("second"), "");
    let (result, requests) = cleanup(vec![first.clone(), second, first]).await;
    assert!(result.unwrap_err().to_string().contains("repeated"));
    assert_eq!(requests.len(), 3);
}

#[tokio::test]
async fn missing_or_empty_key_marker_is_an_error() {
    for key in [None, Some("")] {
        let (result, requests) = cleanup(vec![page(true, key, Some("first"), "")]).await;
        assert!(result.unwrap_err().to_string().contains("next key marker"));
        assert_eq!(requests.len(), 1);
    }
}

#[tokio::test]
async fn advancing_key_without_upload_id_marker_is_valid() {
    let (result, requests) = cleanup(vec![
        page(true, Some("file-next"), None, ""),
        page(false, None, None, ""),
    ])
    .await;
    result.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .any(|request| request.contains("key-marker=file-next"))
    );
}

#[tokio::test]
async fn continuously_changing_markers_are_still_bounded() {
    let pages = (0..1_001)
        .map(|index| page(true, Some("file"), Some(&index.to_string()), ""))
        .collect();
    let (result, requests) = cleanup(pages).await;
    assert!(result.unwrap_err().to_string().contains("page limit"));
    assert_eq!(requests.len(), 1_000);
}
