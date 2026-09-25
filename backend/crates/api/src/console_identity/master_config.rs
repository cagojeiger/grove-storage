use super::secrets;
use grove_management_service::master::Config;
use std::sync::Arc;

pub fn load(env: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<Option<Arc<Config>>> {
    let root_token = env("GROVE_ROOT_TOKEN");
    let root_generation = env("GROVE_ROOT_GENERATION");
    let legacy_token = env("FILEGATE_MASTER_TOKEN");
    let legacy_generation = env("FILEGATE_MASTER_GENERATION");
    anyhow::ensure!(
        !(root_token.is_some() || root_generation.is_some())
            || (legacy_token.is_none() && legacy_generation.is_none()),
        "Configure only GROVE_ROOT_* or legacy FILEGATE_MASTER_*, not both"
    );
    let token = root_token.or(legacy_token);
    let generation = root_generation.or(legacy_generation);
    let (token, generation) = match (token, generation) {
        (None, None) => return Ok(None),
        (Some(token), Some(generation)) => (token, generation),
        _ => anyhow::bail!(
            "Root token and generation must be set together (GROVE_ROOT_* or legacy FILEGATE_MASTER_*)"
        ),
    };
    anyhow::ensure!(
        secrets::valid(&token, secrets::MASTER_PREFIX)
            || secrets::valid(&token, secrets::ROOT_PREFIX),
        "Root token must use gsrt_ (or legacy gsmt_) followed by 64 lowercase hex digits"
    );
    let generation: i64 = generation
        .parse()
        .map_err(|_| anyhow::anyhow!("Root generation must be a positive integer"))?;
    let config = Config::new(generation, secrets::master_hash(&token))
        .map_err(|_| anyhow::anyhow!("Root generation must be a positive integer"))?;
    Ok(Some(Arc::new(config)))
}
