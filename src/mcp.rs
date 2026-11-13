use axum::Router;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::CallToolResult,
    model::ContentBlock,
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ErrorData as McpError, ServerHandler,
};
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{lookup_charts, ChartsHashMaps, ChartsHost};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetChartsArgs {
    #[schemars(description = "Airport identifier code (ICAO or FAA)")]
    airport: String,
}

#[derive(Clone)]
pub struct Server {
    hashmaps: Arc<RwLock<ChartsHashMaps>>,
}

#[tool_router]
impl Server {
    pub const fn from_hashmap(hashmaps: Arc<RwLock<ChartsHashMaps>>) -> Self {
        Self { hashmaps }
    }

    #[tool(description = "Retrieve airport reference chart metadata for a given airport")]
    fn get_charts(
        &self,
        Parameters(GetChartsArgs { airport }): Parameters<GetChartsArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            lookup_charts(&airport, ChartsHost::Faa, &self.hashmaps).map_or_else(
                || CallToolResult::error(vec![ContentBlock::text("Not found")]),
                |dto_vec| {
                    let contents = dto_vec
                        .into_iter()
                        .map(|dto| ContentBlock::text(format!("{dto:?}")))
                        .collect();
                    CallToolResult::success(contents)
                },
            ),
        )
    }
}

#[tool_handler(
    name = "chartsapi-rs",
    version = "0.1.1",
    instructions = "Use this Server to query FAA airport charts for a given airport."
)]
impl ServerHandler for Server {}

pub fn get_router(hashmaps: Arc<RwLock<ChartsHashMaps>>) -> (CancellationToken, Router) {
    let ct = CancellationToken::new();
    let config = StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token());
    let service = StreamableHttpService::new(
        move || Ok(Server::from_hashmap(hashmaps.clone())),
        LocalSessionManager::default().into(),
        config,
    );
    let router = Router::new().nest_service("/mcp", service);
    (ct, router)
}
