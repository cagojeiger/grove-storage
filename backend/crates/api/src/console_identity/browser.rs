use super::{failure, secrets};
use crate::routes::AppState;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, header},
    middleware::Next,
    response::Response,
};
use grove_management_service::{Error, sessions};

pub(super) const COOKIE: &str = "__Host-grove_session";

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, Error> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|value| value.to_str().map_err(|_| Error::Forbidden))
        .transpose()?;
    if values.next().is_some() {
        return Err(Error::Forbidden);
    }
    Ok(value)
}

fn check(state: &AppState, headers: &HeaderMap, method: &Method) -> Result<(), Error> {
    let origin = state.console_origin.as_deref().ok_or(Error::NotFound)?;
    // Proxy identity and Bearer credentials cannot turn into console authority.
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(Error::Unauthenticated);
    }
    let supplied = single(headers, "origin")?;
    if supplied.is_some_and(|value| value != origin) {
        return Err(Error::Forbidden);
    }
    if single(headers, "sec-fetch-site")?
        .is_some_and(|value| !matches!(value, "same-origin" | "none"))
    {
        return Err(Error::Forbidden);
    }
    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
        && (supplied != Some(origin) || single(headers, "x-grove-csrf")? != Some("1"))
    {
        return Err(Error::Forbidden);
    }
    Ok(())
}

pub(super) async fn guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let mut response = match check(&state, request.headers(), request.method()) {
        Ok(()) => next.run(request).await,
        Err(error) => {
            let id = if error == Error::NotFound {
                uuid::Uuid::new_v4()
            } else {
                sessions::reject_browser(&state.pool, error).await
            };
            failure(error, id)
        }
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

pub(super) fn cookie(headers: &HeaderMap) -> Option<&str> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        for pair in value.to_str().ok()?.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name == COOKIE {
                if found.is_some() || !secrets::valid(value, secrets::SESSION_PREFIX) {
                    return None;
                }
                found = Some(value);
            }
        }
    }
    found
}

pub(super) fn set_cookie(response: &mut Response, raw: &str, seconds: i64) {
    let cookie =
        format!("{COOKIE}={raw}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={seconds}");
    if let Ok(value) = cookie.parse() {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
}
