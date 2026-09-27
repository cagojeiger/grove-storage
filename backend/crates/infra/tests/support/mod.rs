#![allow(clippy::unwrap_used)]
use axum::{Router, body::Body, extract::State, http::Request, routing::any};
use filegate_infra::{S3StorageSpec, backend::StorageBackend};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

pub struct Provider {
    pub backend: StorageBackend,
    pub requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Provider {
    pub async fn start(fail: bool) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let router = Router::new()
            .fallback(any(
                move |State(requests): State<Arc<Mutex<Vec<String>>>>, req: Request<Body>| async move {
                    let method = req.method().to_string();
                    let upload = req.uri().query().is_some_and(|q| q.contains("uploadId="));
                    requests.lock().unwrap().push(format!(
                        "{method} {}{}", req.uri().path(),
                        if upload { " (multipart)" } else { "" }
                    ));
                    // Drain the SDK's signed streaming body before replying.
                    axum::body::to_bytes(req.into_body(), 1024).await.unwrap();
                    let failed = fail && (method == "PUT" || upload);
                    let status = if failed { 403 } else { 200 };
                    let body = if failed {
                        "<Error><Code>AccessDenied</Code></Error>"
                    } else {
                        ""
                    };
                    axum::http::Response::builder()
                        .status(status)
                        .header("etag", "\"abc123\"")
                        .header("content-length", if method == "HEAD" { 7 } else { body.len() }.to_string())
                        .body(Body::from(body))
                        .unwrap()
                },
            ))
            .with_state(captured);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            backend: StorageBackend {
                spec: S3StorageSpec {
                    endpoint,
                    public_endpoint: "http://public.invalid".into(),
                    region: "local".into(),
                    bucket: "objects".into(),
                    force_path_style: true,
                    access_key: "access".into(),
                    secret_key: secrecy::SecretString::from("secret".to_owned()),
                },
                force_relay: false,
            },
            requests,
            task,
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}
