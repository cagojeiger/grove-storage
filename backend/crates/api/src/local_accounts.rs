use std::io::IsTerminal;

use grove_core::{ExposeSecret, SecretString};
use grove_management_service::{local_accounts as service, passwords};
use uuid::Uuid;

const USAGE: &str = "usage: grove-storage account init <username> <display-name> | grove-storage account recover <account-id> <username> --yes\nPasswords are entered privately at the terminal. GROVE_DATABASE_URL is required.";

#[derive(Debug, PartialEq, Eq)]
enum Command<'a> {
    Initialize { username: &'a str, name: &'a str },
    Recover { account: Uuid, username: &'a str },
    Help,
}

fn parse<'a>(args: &[&'a str]) -> anyhow::Result<Command<'a>> {
    Ok(match args {
        ["init", username, name] => Command::Initialize { username, name },
        ["recover", account, username, "--yes"] => Command::Recover {
            account: account
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid account ID"))?,
            username,
        },
        [] | ["--help"] | ["-h"] => Command::Help,
        _ => anyhow::bail!("{USAGE}"),
    })
}

pub async fn run() -> anyhow::Result<std::process::ExitCode> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let command = parse(&args)?;
    let username = match &command {
        Command::Help => {
            println!("{USAGE}");
            return Ok(std::process::ExitCode::SUCCESS);
        }
        Command::Initialize { username, name } => {
            anyhow::ensure!(
                !name.trim().is_empty() && name.trim().chars().count() <= 80,
                "display name must contain 1-80 characters"
            );
            *username
        }
        Command::Recover { username, .. } => *username,
    };
    let username =
        passwords::username(username).map_err(|_| anyhow::anyhow!("invalid username"))?;
    let url = SecretString::from(
        std::env::var("GROVE_DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("GROVE_DATABASE_URL is required"))?,
    );
    anyhow::ensure!(
        std::io::stdin().is_terminal(),
        "use an interactive terminal for hidden password input"
    );
    if let Command::Recover { account, .. } = command {
        eprintln!(
            "Recover account {account}. Its browser sessions and management API tokens will be revoked."
        );
    }
    eprintln!("Choose a unique passphrase of 15-128 characters.");
    let password = SecretString::from(rpassword::prompt_password("New password: ")?);
    let confirmation = SecretString::from(rpassword::prompt_password("Confirm password: ")?);
    anyhow::ensure!(
        password.expose_secret() == confirmation.expose_secret(),
        "passwords do not match"
    );
    drop(confirmation);
    let pool = grove_db::connect(url.expose_secret(), 2)
        .await
        .map_err(|_| anyhow::anyhow!("database connection failed"))?;
    grove_db::migrate(&pool)
        .await
        .map_err(|_| anyhow::anyhow!("database migration failed"))?;
    let request_id = Uuid::new_v4();
    let result = match command {
        Command::Initialize { name, .. } => {
            service::initialize(&pool, request_id, &username, name, password).await
        }
        Command::Recover { account, .. } => {
            service::recover(&pool, request_id, account, &username, password)
                .await
                .map(|()| account)
        }
        Command::Help => return Ok(std::process::ExitCode::SUCCESS),
    };
    pool.close().await;
    let account = result.map_err(|error| {
        anyhow::anyhow!(
            "account operation failed: {} (request {request_id})",
            error.code()
        )
    })?;
    println!(
        "{}",
        serde_json::json!({"account_id": account, "username": username, "request_id": request_id})
    );
    Ok(std::process::ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn local_commands_require_explicit_recovery_and_never_accept_password_arguments() {
        let id = Uuid::new_v4().to_string();
        assert_eq!(
            parse(&["init", "owner", "Owner"]).unwrap(),
            Command::Initialize {
                username: "owner",
                name: "Owner"
            }
        );
        assert!(parse(&["recover", &id, "owner", "--yes"]).is_ok());
        for args in [
            vec!["recover", &id, "owner"],
            vec!["init", "owner", "Owner", "password"],
            vec!["recover", "not-a-uuid", "owner", "--yes"],
            vec!["token", "create"],
        ] {
            assert!(parse(&args).is_err());
        }
    }
}
