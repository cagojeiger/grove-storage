#![allow(clippy::unwrap_used)]
use grove_object_service::multipart_commit::MultipartCommit;
use std::{
    future::{Future, ready},
    sync::Mutex,
};

pub struct Fake {
    pub prepared: Result<Option<&'static str>, &'static str>,
    pub completed: Result<Option<String>, &'static str>,
    pub finalized: Result<bool, &'static str>,
    pub calls: Mutex<Vec<String>>,
}

impl Fake {
    pub fn new() -> Self {
        Self {
            prepared: Ok(Some("claimed-snapshot")),
            completed: Ok(Some("verified-etag".into())),
            finalized: Ok(true),
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl MultipartCommit for Fake {
    type Error = &'static str;
    type Prepared = &'static str;

    fn prepare(&self) -> impl Future<Output = Result<Option<Self::Prepared>, Self::Error>> + Send {
        self.calls.lock().unwrap().push("prepare".into());
        ready(self.prepared)
    }

    fn complete(
        &self,
        snapshot: &Self::Prepared,
    ) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send {
        self.calls
            .lock()
            .unwrap()
            .push(format!("complete:{snapshot}"));
        ready(self.completed.clone())
    }

    fn finalize(&self, etag: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        self.calls.lock().unwrap().push(format!("finalize:{etag}"));
        ready(self.finalized)
    }
}
