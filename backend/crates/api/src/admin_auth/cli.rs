use filegate_core::ExposeSecret;
use filegate_db::admin_auth::{self as db, IssueMode};

pub async fn run() -> anyhow::Result<std::process::ExitCode> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    enum Command<'a> {
        Issue(IssueMode, &'a str),
        List,
        Revoke(uuid::Uuid),
    }
    let command = match args.as_slice() {
        ["init"] => Command::Issue(IssueMode::Initialize, "initial-admin"),
        ["recover", "--yes"] => Command::Issue(IssueMode::Recover, "recovery-admin"),
        ["token", "create", label] if !label.trim().is_empty() && label.chars().count() <= 80 => {
            Command::Issue(IssueMode::Create, label)
        }
        ["token", "list"] => Command::List,
        ["token", "revoke", id, "--yes"] => Command::Revoke(id.parse()?),
        _ => anyhow::bail!(
            "usage: filegate admin init | recover --yes | token create <label> | token list | token revoke <id> --yes"
        ),
    };
    let config = filegate_core::Config::load()?;
    let pool = filegate_db::connect(config.database.url.expose_secret(), 2).await?;
    filegate_db::migrate(&pool).await?;
    match command {
        Command::Issue(mode, label) => {
            let raw = format!("fgop_{}", filegate_core::generate_url_secret());
            let credential = db::issue(&pool, mode, label, &super::hash("admin-token", &raw)).await?
                .ok_or_else(|| anyhow::anyhow!("initialization state does not permit this command; init runs once, create/recover require initialization"))?;
            eprintln!("Store this token securely. It is displayed only once. Expires in 90 days.");
            println!(
                "{}",
                serde_json::json!({"id":credential.id, "label":credential.label,
                "expires_at":credential.expires_at, "token":raw})
            );
        }
        Command::List => {
            let credentials: Vec<_> = db::list(&pool).await?.into_iter().map(|c|
                serde_json::json!({"id":c.id,"label":c.label,"expires_at":c.expires_at,"revoked_at":c.revoked_at})).collect();
            println!("{}", serde_json::to_string_pretty(&credentials)?);
        }
        Command::Revoke(id) => {
            anyhow::ensure!(db::revoke(&pool, id).await?, "credential not found");
            println!("{}", serde_json::json!({"revoked":id}));
        }
    }
    pool.close().await;
    Ok(std::process::ExitCode::SUCCESS)
}
