use super::failure;
use axum::{
    Json,
    extract::{
        Path, Query,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    response::Response,
};
use grove_management_service::{Error, Page};
use serde::Deserialize;
use uuid::Uuid;

pub(super) type Id = Result<Path<Uuid>, PathRejection>;
pub(super) type Body<T> = Result<Json<T>, JsonRejection>;
pub(super) type QueryPage = Result<Query<Pagination>, QueryRejection>;

pub(super) fn invalid() -> Response {
    failure(Error::InvalidInput, Uuid::new_v4())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pagination {
    pub before: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u16,
}
fn default_limit() -> u16 {
    50
}
impl Pagination {
    pub fn page<K: std::str::FromStr>(&self) -> Result<Page<K>, Error> {
        let before = self
            .before
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(|_| Error::InvalidInput)?;
        Page::new(before, self.limit).map_err(Error::from)
    }
    pub fn history_page(&self) -> Result<Page<i64>, Error> {
        if self
            .before
            .as_ref()
            .is_some_and(|s| s.parse::<i64>().map_or(true, |n| n <= 0))
        {
            return Err(Error::InvalidInput);
        }
        self.page()
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Role {
    Reader,
    Writer,
    Admin,
}
impl From<Role> for grove_management_policy::Role {
    fn from(role: Role) -> Self {
        match role {
            Role::Reader => Self::Reader,
            Role::Writer => Self::Writer,
            Role::Admin => Self::Admin,
        }
    }
}
pub(super) fn valid_label(value: &str) -> bool {
    (1..=80).contains(&value.trim().chars().count())
}
