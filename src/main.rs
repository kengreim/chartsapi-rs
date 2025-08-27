#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use crate::faa_metafile::{DigitalTpp, ProductSet};
use crate::response_dtos::ResponseDto::{Charts, GroupedCharts};
use crate::response_dtos::{ChartDto, ChartGroup, GroupedChartsDto, ResponseDto};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{NaiveDate, NaiveDateTime, Utc};
use indexmap::IndexMap;

use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};
mod faa_metafile;
#[cfg(feature = "mcp")]
mod mcp;
mod response_dtos;

const DTPP_BASE: &str = "https://aeronav.faa.gov/d-tpp";

struct ChartsHashMaps {
    faa: IndexMap<String, Vec<ChartDto>>,
    icao: IndexMap<String, String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,chartsapi_rs=debug")
        .init();

    // Initialize current_cycle and in-memory hashmaps for FAA/ICAO id lookup
    let current_cycle = RwLock::new(fetch_current_cycle().await.unwrap_or_else(|e| {
        warn!(
            "Error initializing current cycle, falling back to default: {}",
            e
        );
        "2508".to_string()
    }));
    let cycle_clone = current_cycle.read().unwrap().clone();
    let hashmaps = Arc::new(RwLock::new(
        load_charts(&cycle_clone)
            .await
            .expect("Could not fetch and initialize charts"),
    ));
    let axum_state = Arc::clone(&hashmaps);
    #[cfg(feature = "mcp")]
    let mcp_state = Arc::clone(&hashmaps);

    // Spawn cycle and chart update loop
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            match fetch_current_cycle().await {
                Ok(fetched_cycle) => {
                    if fetched_cycle.eq_ignore_ascii_case(&current_cycle.read().unwrap()) {
                        debug!("No new cycle found");
                        continue;
                    }

                    info!("Found new cycle: {fetched_cycle}");
                    match load_charts(&fetched_cycle).await {
                        Ok(new_charts) => {
                            *hashmaps.write().unwrap() = new_charts;
                            *current_cycle.write().unwrap() = fetched_cycle;
                        }
                        Err(e) => warn!("Error while fetching charts: {}", e),
                    }
                }
                Err(e) => warn!("Error while fetching current cycle: {}", e),
            }
        }
    });

    // Create and run axum app
    let base_app = Router::new()
        .route("/v1/charts", get(charts_handler))
        .nest_service("/v1/charts/static", ServeDir::new("assets"))
        .route(
            "/v1/charts/{apt_id}/{chart_search_term}",
            get(chart_search_handler),
        )
        .route("/health", get(|| async {}))
        .with_state(axum_state)
        .layer(TraceLayer::new_for_http());

    #[cfg(feature = "mcp")]
    {
        let (cancellation_token, mcp_router) = mcp::get_router(mcp_state.clone());
        let app = Router::new().merge(mcp_router).merge(base_app);
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
        axum::serve(listener, app).await.unwrap();
        cancellation_token.cancel();
    }
    #[cfg(not(feature = "mcp"))]
    {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
        axum::serve(listener, base_app).await.unwrap();
    }
}

#[derive(Deserialize)]
struct ChartsOptions {
    apt: Option<String>,
    group: Option<i32>,
    host: Option<String>,
}

#[derive(Debug, Copy, Clone)]
pub enum ChartsHost {
    Faa,
    Mirror,
}

impl ChartsHost {
    #[must_use]
    pub fn try_from(string: &Option<String>) -> Option<Self> {
        string.as_ref().and_then(|string| {
            if string.to_uppercase() == "MIRROR" {
                Some(Self::Mirror)
            } else if string.to_uppercase() == "FAA" {
                Some(Self::Faa)
            } else {
                None
            }
        })
    }

    #[must_use]
    pub const fn get_host_base_url(&self) -> &'static str {
        match self {
            Self::Faa => DTPP_BASE,
            Self::Mirror => "https://aeronav.flightsimapi.com/d-tpp",
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ErrorMessage {
    pub status: &'static str,
    pub status_code: &'static str,
    pub message: &'static str,
}

async fn charts_handler(
    State(hashmaps): State<Arc<RwLock<ChartsHashMaps>>>,
    options: Query<ChartsOptions>,
) -> Response {
    let Query(chart_options) = options;

    // Check that we have an airport to lookup
    if chart_options.apt.is_none()
        || chart_options
            .apt
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
    {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorMessage {
                status: "error",
                status_code: "404",
                message: "Please specify an airport.",
            }),
        )
            .into_response();
    }

    // Check if supplied chart group is valid, if given as param
    if chart_options.group.is_some_and(|i| !(1..=7).contains(&i)) {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorMessage {
                status: "error",
                status_code: "403",
                message: "That is not a valid grouping code.",
            }),
        )
            .into_response();
    }

    let host = ChartsHost::try_from(&chart_options.host).unwrap_or(ChartsHost::Faa);
    let mut results: IndexMap<String, ResponseDto> = IndexMap::new();
    for airport in chart_options.apt.unwrap().split(',') {
        let airport_uppercase = airport.to_uppercase();
        if let Some(charts) = lookup_charts(&airport_uppercase, host, &hashmaps) {
            results.insert(
                airport_uppercase,
                apply_group_param(&charts, chart_options.group),
            );
        }
    }
    (StatusCode::OK, Json(results)).into_response()
}

fn lookup_charts(
    apt_id: &str,
    host: ChartsHost,
    hashmaps: &Arc<RwLock<ChartsHashMaps>>,
) -> Option<Vec<ChartDto>> {
    let reader = hashmaps.read().unwrap();
    reader.faa.get(apt_id).map_or_else(
        || {
            reader.icao.get(&apt_id.to_uppercase()).and_then(|faa_id| {
                reader
                    .faa
                    .get(faa_id)
                    .cloned()
                    .map(|charts| set_host_for_charts(charts, host))
            })
        },
        |charts| Some(set_host_for_charts(charts.clone(), host)),
    )
}

#[allow(dead_code)]
fn set_host_for_chart_pdf(mut chart: ChartDto, host: ChartsHost) -> ChartDto {
    chart.pdf_path = format!("{}/{}", host.get_host_base_url(), chart.pdf_path);
    chart
}

fn set_host_for_charts(mut charts: Vec<ChartDto>, host: ChartsHost) -> Vec<ChartDto> {
    for chart in &mut charts {
        chart.pdf_path = format!("{}/{}", host.get_host_base_url(), chart.pdf_path);
    }
    charts
}

#[derive(Deserialize)]
struct ChartsSearchOptions {
    host: Option<String>,
}
async fn chart_search_handler(
    State(hashmaps): State<Arc<RwLock<ChartsHashMaps>>>,
    Path((apt_id, chart_search)): Path<(String, String)>,
    options: Query<ChartsSearchOptions>,
) -> Response {
    let host = ChartsHost::try_from(&options.host).unwrap_or(ChartsHost::Faa);
    if let Some(charts) = lookup_charts(&apt_id.to_uppercase(), host, &hashmaps) {
        if let Some(chart) = charts
            .iter()
            .find(|c| c.chart_name.contains(&chart_search.to_uppercase()))
        {
            return Redirect::temporary(&chart.pdf_path).into_response();
        }

        let cleaned_search: String = chart_search.chars().filter(|c| c.is_alphabetic()).collect();
        if let Some(chart) = charts.iter().find(|c| {
            (c.chart_group == ChartGroup::Arrivals || c.chart_group == ChartGroup::Departures)
                && c.chart_name.contains(&cleaned_search.to_uppercase())
        }) {
            return Redirect::temporary(&chart.pdf_path).into_response();
        }
    }

    // Return 404 if we didn't find a chart above
    (
        StatusCode::NOT_FOUND,
        Json(ErrorMessage {
            status: "error",
            status_code: "404",
            message: "Chart not found.",
        }),
    )
        .into_response()
}

const GROUP_1_TYPES: [ChartGroup; 5] = [
    ChartGroup::Apd,
    ChartGroup::General,
    ChartGroup::Departures,
    ChartGroup::Arrivals,
    ChartGroup::Approaches,
];
const GROUP_2_TYPES: [ChartGroup; 1] = [ChartGroup::Apd];
const GROUP_3_TYPES: [ChartGroup; 2] = [ChartGroup::Apd, ChartGroup::General];
const GROUP_4_TYPES: [ChartGroup; 1] = [ChartGroup::Departures];
const GROUP_5_TYPES: [ChartGroup; 1] = [ChartGroup::Arrivals];
const GROUP_6_TYPES: [ChartGroup; 1] = [ChartGroup::Approaches];
const GROUP_7_TYPES: [ChartGroup; 3] = [
    ChartGroup::Departures,
    ChartGroup::Arrivals,
    ChartGroup::Approaches,
];

fn apply_group_param(charts: &[ChartDto], group: Option<i32>) -> ResponseDto {
    group.map_or_else(
        || Charts(charts.to_owned()),
        |i| match i {
            1 => filter_group_by_types(charts, &GROUP_1_TYPES, true),
            2 => filter_group_by_types(charts, &GROUP_2_TYPES, false),
            3 => filter_group_by_types(charts, &GROUP_3_TYPES, false),
            4 => filter_group_by_types(charts, &GROUP_4_TYPES, false),
            5 => filter_group_by_types(charts, &GROUP_5_TYPES, false),
            6 => filter_group_by_types(charts, &GROUP_6_TYPES, false),
            7 => filter_group_by_types(charts, &GROUP_7_TYPES, true),
            _ => Charts(vec![]),
        },
    )
}

fn filter_group_by_types(
    charts: &[ChartDto],
    types: &[ChartGroup],
    return_groups: bool,
) -> ResponseDto {
    if return_groups {
        let mut grouped = GroupedChartsDto::new();
        charts
            .iter()
            .filter(|c| types.contains(&c.chart_group))
            .for_each(|c| grouped.add_chart(c.clone()));
        GroupedCharts(grouped)
    } else {
        Charts(
            charts
                .iter()
                .filter(|c| types.contains(&c.chart_group))
                .cloned()
                .collect(),
        )
    }
}

async fn load_charts(current_cycle: &str) -> Result<ChartsHashMaps, anyhow::Error> {
    debug!("Starting charts metafile request");
    let metafile_url = format!(
        "{}/xml_data/d-tpp_Metafile.xml",
        faa_cycle_url(current_cycle)
    );
    let metafile = reqwest::get(&metafile_url).await?.text().await?;
    debug!("Charts metafile request completed");
    let dtpp = from_str::<DigitalTpp>(&metafile)?;

    let eff_start =
        NaiveDateTime::parse_from_str(&dtpp.from_effective_date, "%H%MZ %m/%d/%y")?.and_utc();
    let now = Utc::now();
    debug!("Effective start for charts: {}", eff_start);
    if eff_start > now {
        anyhow::bail!("Effective date {} greater than now {}", eff_start, now);
    }

    let mut faa: IndexMap<String, Vec<ChartDto>> = IndexMap::new();
    let mut icao: IndexMap<String, String> = IndexMap::new();
    let mut count = 0;

    for state in dtpp.states {
        for city in state.cities {
            for airport in city.airports {
                for record in airport
                    .chart_records
                    .into_iter()
                    .filter(|r| r.useraction != "D")
                {
                    let chart_dto = ChartDto {
                        state: state.id.clone(),
                        state_full: state.full_name.clone(),
                        city: city.id.clone(),
                        volume: city.volume.clone(),
                        airport_name: airport.id.clone(),
                        military: airport.military.clone(),
                        faa_ident: airport.apt_ident.clone(),
                        icao_ident: airport.icao_ident.clone(),
                        chart_seq: record.chartseq,
                        chart_name: record.chart_name,
                        pdf_path: format!("{current_cycle}/{pdf}", pdf = record.pdf_name),
                        chart_group: match record.chart_code.as_str() {
                            "IAP" => ChartGroup::Approaches,
                            "ODP" | "DP" | "DAU" => ChartGroup::Departures,
                            "STAR" => ChartGroup::Arrivals,
                            "APD" => ChartGroup::Apd,
                            _ => ChartGroup::General, // Includes "MIN" | "LAH" | "HOT"
                        },
                        chart_code: record.chart_code,
                        pdf_name: record.pdf_name,
                    };

                    if !chart_dto.icao_ident.is_empty() {
                        icao.insert(chart_dto.icao_ident.clone(), chart_dto.faa_ident.clone());
                    }

                    // Prefer the syntax below, but requires a clone in the modify case
                    // faa.entry(chart_dto.faa_ident.clone())
                    //     .and_modify(|charts| charts.push(chart_dto.clone()))
                    //     .or_insert(vec![chart_dto]);

                    if let Some(charts) = faa.get_mut(&chart_dto.faa_ident) {
                        charts.push(chart_dto);
                    } else {
                        faa.insert(chart_dto.faa_ident.clone(), vec![chart_dto]);
                    }

                    count += 1;
                }
            }
        }
    }

    info!("Loaded {count} charts");
    Ok(ChartsHashMaps { faa, icao })
}

async fn fetch_current_cycle() -> Result<String, anyhow::Error> {
    info!("Fetching current cycle");
    let cycle_xml = reqwest::get("https://external-api.faa.gov/apra/dtpp/info")
        .await?
        .text()
        .await?;
    let product_set = from_str::<ProductSet>(&cycle_xml)?;
    let date = NaiveDate::parse_from_str(&product_set.edition.date, "%m/%d/%Y")?;
    let cycle_str = if product_set.edition.number.len() == 2 {
        format!("{}{}", date.format("%y"), product_set.edition.number)
    } else {
        format!("{}0{}", date.format("%y"), product_set.edition.number)
    };
    info!("Found current cycle: {cycle_str}");

    Ok(cycle_str)
}

fn faa_cycle_url(current_cycle: &str) -> String {
    format!("{DTPP_BASE}/{current_cycle}",)
}
