use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use grove_management_command::CommandName;

#[derive(Parser)]
#[command(name = "gscli", version, about = "Grove Storage management CLI")]
pub struct Args {
    /// Management API origin.
    #[arg(long, global = true, env = "GROVE_ENDPOINT", hide_env_values = true)]
    pub endpoint: Option<String>,
    /// Read an Account management token from a file (overrides GROVE_TOKEN).
    #[arg(long, global = true)]
    pub token_file: Option<PathBuf>,
    #[arg(long, global = true, value_enum, default_value = "table")]
    pub output: Output,
    /// Whole-command HTTP deadline in seconds.
    #[arg(long, global = true, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=86400))]
    pub timeout: u64,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Output {
    Table,
    Json,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install the latest stable official CLI release (independent of the server).
    Update {
        /// Check for a newer version without changing the installation.
        #[arg(long)]
        check: bool,
    },
    #[command(name = "__install", hide = true)]
    Install {
        #[arg(long)]
        bin_dir: PathBuf,
    },
    /// Inspect the remote API and registry (not physical storage).
    Status,
    /// Manage registered storage backends.
    #[command(subcommand)]
    Storage(StorageCommand),
    /// Manage registered clients and their storage assignments.
    #[command(subcommand)]
    Client(ClientCommand),
    /// Manage S3 credentials owned by a client.
    #[command(subcommand)]
    Credential(CredentialCommand),
    /// Manage native API keys owned by a client.
    #[command(subcommand)]
    ClientKey(ClientKeyCommand),
    /// Inspect storage and client usage.
    #[command(subcommand)]
    Usage(Usage),
}

#[derive(Subcommand)]
pub enum StorageCommand {
    /// Read or replace resource labels without changing connection settings.
    #[command(subcommand)]
    Metadata(MetadataCommand),
    List,
    /// Test saved S3 bucket access without writing objects or registry settings.
    Test {
        #[arg(value_parser = resource_id)]
        id: String,
    },
    Show {
        #[arg(value_parser = resource_id)]
        id: String,
    },
    Create {
        #[arg(value_parser = resource_id)]
        id: String,
        /// Read the storage specification from a JSON file, or stdin with '-'.
        #[arg(long = "from", value_name = "PATH")]
        from: PathBuf,
    },
    Replace {
        #[arg(value_parser = resource_id)]
        id: String,
        /// Read the complete storage specification from a JSON file, or stdin with '-'.
        #[arg(long = "from", value_name = "PATH")]
        from: PathBuf,
        /// Skip the interactive replacement confirmation.
        #[arg(long)]
        yes: bool,
    },
    Delete {
        #[arg(value_parser = resource_id)]
        id: String,
        /// Skip the interactive deletion confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum ClientCommand {
    /// Read or replace resource labels without changing the storage assignment.
    #[command(subcommand)]
    Metadata(MetadataCommand),
    List,
    Show {
        #[arg(value_parser = resource_id)]
        id: String,
    },
    Create {
        #[arg(value_parser = resource_id)]
        id: String,
        #[arg(long, value_parser = resource_id)]
        storage: String,
    },
    Delete {
        #[arg(value_parser = resource_id)]
        id: String,
        /// Skip the interactive deletion confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum MetadataCommand {
    Show {
        #[arg(value_parser = resource_id)]
        id: String,
    },
    /// Replace the entire string-valued JSON object; {} clears all labels.
    Replace {
        #[arg(value_parser = resource_id)]
        id: String,
        #[arg(long = "from", value_name = "PATH")]
        from: PathBuf,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum CredentialCommand {
    List {
        #[arg(long, value_parser = resource_id)]
        client: String,
    },
    Create {
        #[arg(long, value_parser = resource_id)]
        client: String,
        /// Create a new file containing the one-time credential.
        #[arg(long, value_name = "PATH", value_parser = secret_output_path)]
        secret_out: PathBuf,
    },
    Delete {
        #[arg(long, value_parser = resource_id)]
        client: String,
        #[arg(value_parser = access_key_id)]
        access_key_id: String,
        /// Skip the interactive deletion confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum ClientKeyCommand {
    List {
        #[arg(long, value_parser = resource_id)]
        client: String,
    },
    Register {
        #[arg(long, value_parser = resource_id)]
        client: String,
        /// Read the existing raw key from a file and register its SHA-256 hash.
        #[arg(long, value_name = "PATH")]
        key_file: PathBuf,
    },
    Delete {
        #[arg(long, value_parser = resource_id)]
        client: String,
        #[arg(value_parser = key_hash)]
        key_hash: String,
        /// Skip the interactive deletion confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum Usage {
    Storages,
    Clients,
    History {
        #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u16).range(1..=3650))]
        days: u16,
    },
}

fn resource_id(value: &str) -> Result<String, &'static str> {
    if value.is_empty() || matches!(value, "." | "..") || value.chars().any(char::is_control) {
        return Err("expected a nonempty resource ID without dot segments or control characters");
    }
    Ok(value.to_owned())
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Update { check: true } => "update.check",
            Self::Update { check: false } => "update",
            Self::Install { .. } => "install",
            Self::Status => CommandName::Status.as_str(),
            Self::Storage(StorageCommand::List) => CommandName::StorageList.as_str(),
            Self::Storage(StorageCommand::Metadata(MetadataCommand::Show { .. })) => {
                CommandName::StorageMetadataShow.as_str()
            }
            Self::Storage(StorageCommand::Metadata(MetadataCommand::Replace { .. })) => {
                CommandName::StorageMetadataReplace.as_str()
            }
            Self::Storage(StorageCommand::Show { .. }) => CommandName::StorageShow.as_str(),
            Self::Storage(StorageCommand::Test { .. }) => CommandName::StorageTest.as_str(),
            Self::Storage(StorageCommand::Create { .. }) => CommandName::StorageCreate.as_str(),
            Self::Storage(StorageCommand::Replace { .. }) => CommandName::StorageReplace.as_str(),
            Self::Storage(StorageCommand::Delete { .. }) => CommandName::StorageDelete.as_str(),
            Self::Client(ClientCommand::List) => CommandName::ClientList.as_str(),
            Self::Client(ClientCommand::Metadata(MetadataCommand::Show { .. })) => {
                CommandName::ClientMetadataShow.as_str()
            }
            Self::Client(ClientCommand::Metadata(MetadataCommand::Replace { .. })) => {
                CommandName::ClientMetadataReplace.as_str()
            }
            Self::Client(ClientCommand::Show { .. }) => CommandName::ClientShow.as_str(),
            Self::Client(ClientCommand::Create { .. }) => CommandName::ClientCreate.as_str(),
            Self::Client(ClientCommand::Delete { .. }) => CommandName::ClientDelete.as_str(),
            Self::Credential(CredentialCommand::List { .. }) => {
                CommandName::CredentialList.as_str()
            }
            Self::Credential(CredentialCommand::Create { .. }) => {
                CommandName::CredentialCreate.as_str()
            }
            Self::Credential(CredentialCommand::Delete { .. }) => {
                CommandName::CredentialDelete.as_str()
            }
            Self::ClientKey(ClientKeyCommand::List { .. }) => CommandName::ClientKeyList.as_str(),
            Self::ClientKey(ClientKeyCommand::Register { .. }) => {
                CommandName::ClientKeyRegister.as_str()
            }
            Self::ClientKey(ClientKeyCommand::Delete { .. }) => {
                CommandName::ClientKeyDelete.as_str()
            }
            Self::Usage(Usage::Storages) => CommandName::UsageStorages.as_str(),
            Self::Usage(Usage::Clients) => CommandName::UsageClients.as_str(),
            Self::Usage(Usage::History { .. }) => CommandName::UsageHistory.as_str(),
        }
    }
}

fn key_hash(value: &str) -> Result<String, &'static str> {
    let digest = value.strip_prefix("sha256:").unwrap_or_default();
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("expected sha256: followed by 64 lowercase hexadecimal characters");
    }
    Ok(value.to_owned())
}

fn access_key_id(value: &str) -> Result<String, &'static str> {
    if valid_access_key_id(value) {
        Ok(value.to_owned())
    } else {
        Err("expected 8-64 lowercase ASCII letters or digits")
    }
}

pub(crate) fn valid_access_key_id(value: &str) -> bool {
    (8..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
}

fn secret_output_path(value: &str) -> Result<PathBuf, &'static str> {
    if value == "-" {
        return Err("secret output must be a file path, not stdout");
    }
    Ok(PathBuf::from(value))
}
