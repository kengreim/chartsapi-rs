use axum::Router;
use rmcp::{
    Error as McpError, ServerHandler, const_string,
    model::{
        CallToolResult, ConstString, Content, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    tool,
    transport::{SseServer, sse_server::SseServerConfig},
};
use std::sync::{Arc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{ChartsHashMaps, ChartsHost, lookup_charts};

#[derive(Clone)]
pub struct Server {
    hashmaps: Arc<RwLock<ChartsHashMaps>>,
}
#[tool(tool_box)]
impl Server {
    pub const fn from_hashmap(hashmaps: Arc<RwLock<ChartsHashMaps>>) -> Self {
        Self { hashmaps }
    }

    #[tool(description = "Retrieve airport reference chart metadata for a given airport")]
    fn get_charts(
        &self,
        #[tool(param)]
        #[schemars(description = "Airport identifier code (ICAO or FAA)")]
        airport: String,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            lookup_charts(&airport, ChartsHost::Faa, &self.hashmaps).map_or_else(
                || CallToolResult::error(vec![Content::text("Not found")]),
                |dto_vec| {
                    let contents = dto_vec
                        .into_iter()
                        .map(|dto| Content::text(format!("{dto:?}")))
                        .collect();
                    CallToolResult::success(contents)
                },
            ),
        )
    }
}

const_string!(Echo = "echo");
#[tool(tool_box)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: String::from("chartsapi-rs"),
                version: String::from("0.1.1"),
            },
            instructions: Some(
                "Use this Server to query FAA airport charts for a given airport.".to_string(),
            ),
        }
    }
}

pub fn get_router(hashmaps: Arc<RwLock<ChartsHashMaps>>) -> (CancellationToken, Router) {
    let config = SseServerConfig {
        bind: "0.0.0.0:8000".parse().unwrap(), // NOTE: unused
        sse_path: "/sse".to_string(),
        post_path: "/message".to_string(),
        ct: tokio_util::sync::CancellationToken::new(),
    };
    let (sse_server, router) = SseServer::new(config);
    let ct = sse_server.with_service(move || Server::from_hashmap(hashmaps.clone()));
    (ct, router)
}
