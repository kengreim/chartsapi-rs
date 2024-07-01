#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use crate::faa_metafile::{DigitalTpp, ProductSet};
use crate::response_dtos::ResponseDto::{Charts, GroupedCharts};
use crate::response_dtos::{ChartDto, ChartGroup, GroupedChartsDto, ResponseDto};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use indexmap::IndexMap;
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};

mod faa_metafile;
mod response_dtos;

struct ChartsHashMaps {
    faa: IndexMap<String, Vec<ChartDto>>,
    icao: IndexMap<String, String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Initialize current_cycle and in-memory hashmaps for FAA/ICAO id lookup
    let current_cycle = RwLock::new(fetch_current_cycle().await.unwrap_or_else(|e| {
        warn!(
            "Error initializing current cycle, falling back to default: {}",
            e
        );
        "2406".to_string()
    }));
    let hashmaps = Arc::new(RwLock::new(load_charts(&current_cycle.read().await).await?));
    let axum_state = Arc::clone(&hashmaps);

    // Spawn cycle and chart update loop
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            match fetch_current_cycle().await {
                Ok(fetched_cycle) => {
                    if fetched_cycle.eq_ignore_ascii_case(&current_cycle.read().await) {
                        debug!("No new cycle found");
                        return;
                    }

                    info!("Found new cycle: {fetched_cycle}");
                    match load_charts(&fetched_cycle).await {
                        Ok(new_charts) => {
                            *hashmaps.write().await = new_charts;
                            *current_cycle.write().await = fetched_cycle;
                        }
                        Err(e) => warn!("Error while fetching charts: {}", e),
                    }
                }
                Err(e) => warn!("Error while fetching current cycle: {}", e),
            }
        }
    });

    // Create and run axum app
    let app = Router::new()
        .route("/v1/charts", get(charts_handler))
        .with_state(axum_state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Deserialize)]
struct ChartsOptions {
    apt: Option<String>,
    group: Option<i32>,
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

    let mut results: IndexMap<String, ResponseDto> = IndexMap::new();
    for airport in chart_options.apt.unwrap().split(',') {
        if let Some(charts) = lookup_charts(airport, &hashmaps).await {
            results.insert(
                airport.to_owned(),
                apply_group_param(&charts, chart_options.group),
            );
        }
    }
    (StatusCode::OK, Json(results)).into_response()
}

async fn lookup_charts(
    apt_id: &str,
    hashmaps: &Arc<RwLock<ChartsHashMaps>>,
) -> Option<Vec<ChartDto>> {
    let reader = hashmaps.read().await;
    reader.faa.get(&apt_id.to_uppercase()).map_or_else(
        || {
            reader
                .icao
                .get(&apt_id.to_uppercase())
                .and_then(|faa_id| reader.faa.get(faa_id).cloned())
        },
        |charts| Some(charts.clone()),
    )
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
    let base_url = cycle_url(current_cycle);
    let metafile = reqwest::get(format!("{base_url}/xml_data/d-tpp_Metafile.xml"))
        .await?
        .text()
        .await?;
    debug!("Charts metafile request completed");
    let dtpp = from_str::<DigitalTpp>(&metafile)?;
    let mut faa = IndexMap::new();
    let mut icao = IndexMap::new();
    let mut count = 0;

    for state in dtpp.states {
        for city in state.cities {
            for airport in city.airports {
                for record in airport.chart_records {
                    let chart_dto = ChartDto {
                        state: state.id.to_string(),
                        state_full: state.full_name.to_string(),
                        city: city.id.to_string(),
                        volume: city.volume.to_string(),
                        airport_name: airport.id.to_string(),
                        military: airport.military.to_string(),
                        faa_ident: airport.apt_ident.to_string(),
                        icao_ident: airport.icao_ident.to_string(),
                        chart_seq: record.chartseq.to_string(),
                        chart_code: record.chart_code.to_string(),
                        chart_name: record.chart_name.to_string(),
                        pdf_name: record.pdf_name.to_string(),
                        pdf_path: format!("{base_url}/{pdf}", pdf = record.pdf_name),
                        chart_group: match &*record.chart_code {
                            "IAP" => ChartGroup::Approaches,
                            "ODP" | "DP" | "DAU" => ChartGroup::Departures,
                            "STAR" => ChartGroup::Arrivals,
                            "APD" => ChartGroup::Apd,
                            _ => ChartGroup::General, // Includes "MIN" | "LAH" | "HOT"
                        },
                    };
                    if !chart_dto.icao_ident.is_empty() {
                        icao.insert(chart_dto.icao_ident.clone(), chart_dto.faa_ident.clone());
                    }
                    faa.entry(chart_dto.faa_ident.clone())
                        .and_modify(|charts: &mut Vec<ChartDto>| charts.push(chart_dto.clone()))
                        .or_insert_with(|| vec![chart_dto]);

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
    let cycle_str = date.format("%y%m").to_string();
    info!("Found current cycle: {cycle_str}");
    Ok(cycle_str)
}

fn cycle_url(current_cycle: &str) -> String {
    format!("https://aeronav.faa.gov/d-tpp/{current_cycle}",)
}
