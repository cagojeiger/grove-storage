use super::support::hash;
use filegate_db::PgPool;
use grove_management_service::master::{self, Config};

pub fn config(generation: i64, seed: u64) -> Config {
    Config::new(generation, hash(seed)).unwrap()
}
pub async fn login(pool: &PgPool, config: &Config, token: u64, session: u64) {
    assert!(
        master::login(pool, config, Some(&hash(token)), &hash(session))
            .await
            .result
            .is_ok()
    );
}
