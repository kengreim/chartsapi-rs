use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DigitalTpp {
    #[serde(alias = "@Cycle", alias = "@cycle")]
    pub cycle: String,
    #[serde(rename = "@from_edate")]
    pub from_effective_date: String,
    #[serde(rename = "@to_edate")]
    pub to_effective_date: String,
    #[serde(rename = "state_code")]
    pub states: Vec<State>,
}

#[derive(Serialize, Deserialize)]
pub struct State {
    #[serde(rename = "@ID")]
    pub id: String,
    #[serde(rename = "@state_fullname")]
    pub full_name: String,
    #[serde(rename = "city_name")]
    pub cities: Vec<City>,
}

#[derive(Serialize, Deserialize)]
pub struct City {
    #[serde(rename = "@ID")]
    pub id: String,
    #[serde(rename = "@volume")]
    pub volume: String,
    #[serde(rename = "airport_name")]
    pub airports: Vec<Airport>,
}

#[derive(Serialize, Deserialize)]
pub struct Airport {
    #[serde(rename = "@ID")]
    pub id: String,
    #[serde(rename = "@military")]
    pub military: String,
    #[serde(rename = "@apt_ident")]
    pub apt_ident: String,
    #[serde(rename = "@icao_ident")]
    pub icao_ident: String,
    #[serde(rename = "@alnum")]
    pub alnum: String,
    #[serde(rename = "record")]
    pub chart_records: Vec<ChartRecord>,
}

#[derive(Serialize, Deserialize)]
pub struct ChartRecord {
    pub chartseq: String,
    pub chart_code: String,
    pub chart_name: String,
    pub useraction: String,
    pub pdf_name: String,
    pub cn_flg: String,
    pub cnsection: String,
    pub cnpage: String,
    pub bvsection: String,
    pub bvpage: String,
    pub procuid: String,
    pub two_colored: String,
    pub civil: String,
    pub faanfd18: String,
    pub copter: String,
    pub amdtnum: String,
    pub amdtdate: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProductSet {
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    pub status: Status,
    pub edition: Edition,
}

#[derive(Serialize, Deserialize)]
pub struct Status {
    #[serde(rename = "@code")]
    pub code: String,
    #[serde(rename = "@message")]
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct Edition {
    #[serde(rename = "@geoname")]
    pub geoname: String,
    #[serde(rename = "@editionName")]
    pub name: String,
    #[serde(rename = "@format")]
    pub format: String,
    #[serde(rename = "editionDate")]
    pub date: String,
    #[serde(rename = "editionNumber")]
    pub number: String,
}
