use super::secrets;
use grove_management_service::master::Config;
use std::sync::Arc;

pub fn load(env: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<Option<Arc<Config>>> {
    let token = env("FILEGATE_MASTER_TOKEN");
    let generation = env("FILEGATE_MASTER_GENERATION");
    let (token, generation) = match (token, generation) {
        (None, None) => return Ok(None),
        (Some(token), Some(generation)) => (token, generation),
        _ => anyhow::bail!(
            "FILEGATE_MASTER_TOKEN and FILEGATE_MASTER_GENERATION must be set together"
        ),
    };
    anyhow::ensure!(
        secrets::valid(&token, secrets::MASTER_PREFIX),
        "FILEGATE_MASTER_TOKEN must use gsmt_ followed by 64 lowercase hex digits"
    );
    let generation: i64 = generation
        .parse()
        .map_err(|_| anyhow::anyhow!("FILEGATE_MASTER_GENERATION must be a positive integer"))?;
    let config = Config::new(generation, secrets::master_hash(&token))
        .map_err(|_| anyhow::anyhow!("FILEGATE_MASTER_GENERATION must be a positive integer"))?;
    Ok(Some(Arc::new(config)))
}
