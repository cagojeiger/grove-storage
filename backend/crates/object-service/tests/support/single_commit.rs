#![allow(clippy::unwrap_used)]
use grove_object_service::single_commit::SingleCommit;
use std::{
    future::{Future, ready},
    sync::Mutex,
};

pub struct Fake {
    pub observation: Result<Option<(i64, String)>, &'static str>,
    pub finalized: Result<bool, &'static str>,
    pub current: Result<Option<String>, &'static str>,
    pub calls: Mutex<Vec<String>>,
}

impl Fake {
    pub fn new() -> Self {
        Self {
            observation: Ok(Some((7, "etag".into()))),
            finalized: Ok(true),
            current: Ok(Some("winner".into())),
            calls: Mutex::new(Vec::new()),
        }
    }
    pub fn record(&self, call: impl Into<String>) {
        self.calls.lock().unwrap().push(call.into());
    }
}

impl SingleCommit for Fake {
    type Error = &'static str;

    fn observe(&self) -> impl Future<Output = Result<Option<(i64, String)>, Self::Error>> + Send {
        self.record("observe");
        ready(self.observation.clone())
    }

    fn finalize(&self, etag: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        self.record(format!("finalize:{etag}"));
        ready(self.finalized)
    }

    fn committed_etag(&self) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send {
        self.record("reload");
        ready(self.current.clone())
    }
}
