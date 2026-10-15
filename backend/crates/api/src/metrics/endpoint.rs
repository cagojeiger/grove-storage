use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use grove_db::PgPool;

use super::Metrics;
use crate::spool::SpoolBudget;

pub(crate) fn router(
    metrics: Metrics,
    pool: PgPool,
    maintenance: PgPool,
    observations: Option<PgPool>,
    spool: Arc<SpoolBudget>,
) -> Router {
    let mut pools = vec![("requests", pool), ("maintenance", maintenance)];
    if let Some(pool) = observations {
        pools.push(("observations", pool));
    }
    Router::new()
        .route("/metrics", get(scrape))
        .with_state(Endpoint {
            metrics,
            pools,
            spool,
        })
}

#[derive(Clone)]
struct Endpoint {
    metrics: Metrics,
    pools: Vec<(&'static str, PgPool)>,
    spool: Arc<SpoolBudget>,
}

async fn scrape(State(endpoint): State<Endpoint>) -> axum::response::Response {
    let m = &endpoint.metrics.0;
    for (name, pool) in endpoint.pools {
        let labels = vec![("pool", name)];
        m.pool_connections
            .get_or_create(&labels)
            .set(i64::from(pool.size()));
        m.pool_idle
            .get_or_create(&labels)
            .set(pool.num_idle() as i64);
        m.pool_limit
            .get_or_create(&labels)
            .set(i64::from(pool.options().get_max_connections()));
    }
    let (reserved, capacity, active) = endpoint.spool.reservations();
    m.spool_reserved.set(reserved as i64);
    m.spool_capacity.set(capacity as i64);
    m.spool_active.set(active as i64);
    match endpoint.metrics.render() {
        Ok(body) => (
            [
                (
                    header::CONTENT_TYPE,
                    "application/openmetrics-text; version=1.0.0; charset=utf-8",
                ),
                (header::CACHE_CONTROL, "no-store"),
            ],
            body,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
