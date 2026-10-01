use super::contract;
use crate::routes::AppState;
use grove_management_command::{COMMAND_PROTOCOL_VERSION, CommandName};
use grove_management_policy::Surface;
use grove_management_service::{Proof, resources};
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::{Value, json};

pub(super) struct Server {
    state: AppState,
    token_hash: String,
}

impl Server {
    pub(super) fn new(state: AppState, token_hash: String) -> Self {
        Self { state, token_hash }
    }
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
            .with_server_info(Implementation::new("grove-storage", env!("CARGO_PKG_VERSION")))
            .with_instructions("Manage Storage, Client, and service keys using command protocol 1. Identity, roles, management tokens, and audit browsing belong to the console. Mutations require user intent; check targets before replacement or deletion. On an unknown outcome, inspect current state before retrying. credential.create delivers a one-time S3 secret to this MCP client; use only a trusted client and protect its transcript.")
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if request.and_then(|p| p.cursor).is_some() {
            return Err(ErrorData::invalid_params(
                "Pagination is not supported",
                None,
            ));
        }
        Ok(ListToolsResult::with_all_items(
            CommandName::ALL
                .iter()
                .copied()
                .map(contract::tool)
                .collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let command = grove_management_command::decode(
            COMMAND_PROTOCOL_VERSION,
            &request.name,
            Value::Object(request.arguments.unwrap_or_default()),
        )
        .map_err(|error| {
            ErrorData::invalid_params("Invalid resource command", Some(json!(error)))
        })?;
        let execution = resources::execute(
            &self.state.pool,
            &self.state.crypto,
            |input| {
                crate::storage_registration::verify_command(
                    &self.state.crypto,
                    self.state.public_url.is_some(),
                    input,
                )
            },
            Proof::Token(&self.token_hash),
            Surface::Mcp,
            command,
        )
        .await;
        Ok(match execution.result {
            Ok(output) => CallToolResult::structured(json!({
                "protocol": COMMAND_PROTOCOL_VERSION, "request_id": execution.request_id,
                "command": output.name().as_str(), "result": output,
            })),
            Err(error) => CallToolResult::structured_error(json!({
                "protocol": COMMAND_PROTOCOL_VERSION, "request_id": execution.request_id, "error": error,
            })),
        }.into())
    }
}
