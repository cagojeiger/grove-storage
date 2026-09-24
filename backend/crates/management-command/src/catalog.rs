use grove_management_policy::Action;
use schemars::{Schema, schema_for};
use serde::Serialize;
use serde_json::Value;

use crate::input::{self, Validate};
use crate::{CommandError, ErrorCode, model};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Read,
    Mutation,
}

// A single catalog binds both transports to names, types, and permissions.
macro_rules! commands {
    ($($variant:ident => ($name:literal, $input:ty, $output:ty, $action:ident, $effect:ident)),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CommandName { $($variant),+ }

        pub enum Command { $($variant($input)),+ }

        /// Typed wire results, including the intentional one-time secret reply.
        /// Debug omits payloads; Serialize is for delivery, not audit capture.
        #[derive(Serialize)]
        #[serde(untagged)]
        pub enum Output { $($variant($output)),+ }

        impl CommandName {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub fn parse(name: &str) -> Option<Self> {
                match name { $($name => Some(Self::$variant),)+ _ => None }
            }

            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $name),+ }
            }

            pub const fn required_action(self) -> Action {
                match self { $(Self::$variant => Action::$action),+ }
            }

            pub const fn effect(self) -> Effect {
                match self { $(Self::$variant => Effect::$effect),+ }
            }

            pub fn input_schema(self) -> Schema {
                match self { $(Self::$variant => schema_for!($input)),+ }
            }

            pub fn output_schema(self) -> Schema {
                match self { $(Self::$variant => schema_for!($output)),+ }
            }

            pub(crate) fn decode_input(self, value: Value) -> Result<Command, CommandError> {
                match self {
                    $(Self::$variant => serde_json::from_value::<$input>(value).map(Command::$variant)),+
                }.map_err(|_| CommandError::rejected(ErrorCode::InvalidInput))
            }

            /// Decode shape only. Correlation checks and mutation outcome are
            /// service/adapter responsibilities, not inferred from JSON success.
            pub fn decode_output(self, value: Value) -> Result<Output, ErrorCode> {
                match self {
                    $(Self::$variant => serde_json::from_value::<$output>(value).map(Output::$variant)),+
                }.map_err(|_| ErrorCode::InvalidResponse)
            }
        }

        impl Command {
            pub const fn name(&self) -> CommandName {
                match self { $(Self::$variant(_) => CommandName::$variant),+ }
            }

            pub fn validate(&self) -> Result<(), CommandError> {
                match self { $(Self::$variant(input) => input.validate()),+ }
            }
        }

        impl Output {
            pub const fn name(&self) -> CommandName {
                match self { $(Self::$variant(_) => CommandName::$variant),+ }
            }
        }
    };
}

commands! {
    Status => ("status", input::EmptyInput, model::Status, ReadResources, Read),
    StorageList => ("storage.list", input::EmptyInput, Vec<model::Storage>, ReadResources, Read),
    StorageShow => ("storage.show", input::ResourceInput, model::Storage, ReadResources, Read),
    StorageCreate => ("storage.create", input::StorageInput, model::Storage, WriteResources, Mutation),
    StorageReplace => ("storage.replace", input::StorageInput, model::Storage, WriteResources, Mutation),
    StorageDelete => ("storage.delete", input::ResourceInput, model::Deleted, WriteResources, Mutation),
    ClientList => ("client.list", input::EmptyInput, Vec<String>, ReadResources, Read),
    ClientShow => ("client.show", input::ResourceInput, model::Client, ReadResources, Read),
    ClientCreate => ("client.create", input::ClientCreateInput, model::Client, WriteResources, Mutation),
    ClientDelete => ("client.delete", input::ResourceInput, model::Deleted, WriteResources, Mutation),
    CredentialList => ("credential.list", input::ClientInput, Vec<String>, ManageServiceCredentials, Read),
    CredentialCreate => ("credential.create", input::ClientInput, model::IssuedCredential, ManageServiceCredentials, Mutation),
    CredentialDelete => ("credential.delete", input::CredentialDeleteInput, model::Deleted, ManageServiceCredentials, Mutation),
    ClientKeyList => ("client-key.list", input::ClientInput, Vec<String>, ManageServiceCredentials, Read),
    ClientKeyRegister => ("client-key.register", input::ClientKeyInput, model::ClientKey, ManageServiceCredentials, Mutation),
    ClientKeyDelete => ("client-key.delete", input::ClientKeyInput, model::Deleted, ManageServiceCredentials, Mutation),
    UsageStorages => ("usage.storages", input::EmptyInput, Vec<model::StorageUsage>, ReadResources, Read),
    UsageClients => ("usage.clients", input::EmptyInput, Vec<model::ClientUsage>, ReadResources, Read),
    UsageHistory => ("usage.history", input::HistoryInput, Vec<model::Snapshot>, ReadResources, Read),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output")
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}
