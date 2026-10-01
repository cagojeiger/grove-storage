mod wire;

use crate::error::Error;
use grove_management_command::{COMMAND_PROTOCOL_VERSION, Command, Effect, Output};
use reqwest::{Client, Url, header::HeaderValue};
use tokio::time::{Instant, timeout_at};

const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

pub struct Api {
    client: Client,
    endpoint: Url,
    authorization: HeaderValue,
    deadline: Instant,
}

impl Api {
    pub fn new(endpoint: Url, authorization: HeaderValue, seconds: u64) -> Result<Self, Error> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .user_agent(concat!("gscli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error::new("http_client", "Cannot initialize HTTPS client", 1))?;
        Ok(Self {
            client,
            endpoint,
            authorization,
            deadline: Instant::now() + std::time::Duration::from_secs(seconds),
        })
    }

    pub fn origin(&self) -> String {
        self.endpoint.origin().ascii_serialization()
    }

    pub async fn execute(&self, command: Command) -> Result<Output, Error> {
        command
            .validate()
            .map_err(|_| Error::input("Invalid command input"))?;
        let mutation = command.name().effect() == Effect::Mutation;
        let mut url = self.endpoint.clone();
        url.set_path("/api/admin/commands/v1");
        let body = serde_json::to_vec(&serde_json::json!({
            "protocol":COMMAND_PROTOCOL_VERSION,"command":command.name().as_str(),"input":command,
        }))
        .map_err(|_| Error::input("Cannot encode the request body"))?;
        if Instant::now() >= self.deadline {
            return Err(Error::timeout());
        }
        let operation = async {
            let mut response = self
                .client
                .post(url)
                .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(reqwest::header::ACCEPT, "application/json")
                .body(body)
                .send()
                .await
                .map_err(|_| {
                    if mutation {
                        Error::mutation_transport()
                    } else {
                        Error::new("transport", "HTTP connection, TLS, or transport failure", 5)
                    }
                })?;
            let status = response.status().as_u16();
            if response.status().is_redirection() {
                return Err(Error::untrusted_response(status, mutation));
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| Error::unverified_result(status, mutation))?
            {
                if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                    return Err(Error::response_too_large(status, mutation));
                }
                bytes.extend_from_slice(&chunk);
            }
            wire::decode(&command, status, &bytes)
        };
        match timeout_at(self.deadline, operation).await {
            Ok(result) => result,
            Err(_) if mutation => Err(Error::mutation_timeout()),
            Err(_) => Err(Error::timeout()),
        }
    }
}
