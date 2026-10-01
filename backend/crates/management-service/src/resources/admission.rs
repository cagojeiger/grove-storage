use super::*;

/// Discovery also requires a current identity. Tool execution resolves the
/// proof again under its own transaction; admission never caches authority.
pub async fn admit_mcp(pool: &PgPool, token_hash: &str) -> Result<(), (Uuid, CommandError)> {
    let request_id = Uuid::new_v4();
    let started = std::time::Instant::now();
    let mut context = None;
    let result = async {
        let mut tx = IdentityTransaction::begin(pool).await?;
        current(
            &mut tx,
            Proof::Token(token_hash),
            Surface::Mcp,
            grove_management_command::CommandName::Status,
            request_id,
            &mut context,
        )
        .await?;
        tx.finish().await.map_err(|_| Error::Unavailable)
    }
    .await;
    if result.is_err() {
        logging::record(
            pool,
            context.as_ref(),
            request_id,
            Surface::Mcp,
            "mcp.connect",
            started.elapsed(),
            &result,
        )
        .await;
    }
    result.map_err(|error| (request_id, wire_error(error)))
}
