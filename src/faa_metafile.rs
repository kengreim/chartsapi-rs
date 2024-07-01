use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DigitalTpp<'a> {
    #[serde(rename = "@cycle")]
    pub cycle: Cow<'a, str>,
    #[serde(rename = "@from_edate")]
    pub from_effective_date: Cow<'a, str>,
    #[serde(rename = "@to_edate")]
    pub to_effective_date: Cow<'a, str>,
    #[serde(rename = "state_code")]
    #[serde(borrow)]
    pub states: Vec<State<'a>>,
}

#[derive(Serialize, Deserialize)]
pub struct State<'a> {
    #[serde(rename = "@ID")]
    pub id: Cow<'a, str>,
    #[serde(rename = "@state_fullname")]
    pub full_name: Cow<'a, str>,
    #[serde(rename = "city_name")]
    #[serde(borrow)]
    pub cities: Vec<City<'a>>,
}

#[derive(Serialize, Deserialize)]
pub struct City<'a> {
    #[serde(rename = "@ID")]
    pub id: Cow<'a, str>,
    #[serde(rename = "@volume")]
    pub volume: Cow<'a, str>,
    #[serde(rename = "airport_name")]
    #[serde(borrow)]
    pub airports: Vec<Airport<'a>>,
}

#[derive(Serialize, Deserialize)]
pub struct Airport<'a> {
    #[serde(rename = "@ID")]
    pub id: Cow<'a, str>,
    #[serde(rename = "@military")]
    pub military: Cow<'a, str>,
    #[serde(rename = "@apt_ident")]
    pub apt_ident: Cow<'a, str>,
    #[serde(rename = "@icao_ident")]
    pub icao_ident: Cow<'a, str>,
    #[serde(rename = "@alnum")]
    pub alnum: Cow<'a, str>,
    #[serde(rename = "record")]
    #[serde(borrow)]
    pub chart_records: Vec<ChartRecord<'a>>,
}

#[derive(Serialize, Deserialize)]
pub struct ChartRecord<'a> {
    pub chartseq: Cow<'a, str>,
    pub chart_code: Cow<'a, str>,
    pub chart_name: Cow<'a, str>,
    pub useraction: Cow<'a, str>,
    pub pdf_name: Cow<'a, str>,
    pub cn_flg: Cow<'a, str>,
    pub cnsection: Cow<'a, str>,
    pub cnpage: Cow<'a, str>,
    pub bvsection: Cow<'a, str>,
    pub bvpage: Cow<'a, str>,
    pub procuid: Cow<'a, str>,
    pub two_colored: Cow<'a, str>,
    pub civil: Cow<'a, str>,
    pub faanfd18: Cow<'a, str>,
    pub copter: Cow<'a, str>,
    pub amdtnum: Cow<'a, str>,
    pub amdtdate: Cow<'a, str>,
}

#[derive(Serialize, Deserialize)]
pub struct ProductSet<'a> {
    #[serde(rename = "@xmlns")]
    pub xmlns: Cow<'a, str>,
    pub status: Cow<'a, str>,
    #[serde(borrow)]
    pub edition: Edition<'a>,
}

#[derive(Serialize, Deserialize)]
pub struct Status<'a> {
    #[serde(rename = "@code")]
    pub code: Cow<'a, str>,
    #[serde(rename = "@message")]
    pub message: Cow<'a, str>,
}

#[derive(Serialize, Deserialize)]
pub struct Edition<'a> {
    #[serde(rename = "@geoname")]
    pub geoname: Cow<'a, str>,
    #[serde(rename = "@editionName")]
    pub name: Cow<'a, str>,
    #[serde(rename = "@format")]
    pub format: Cow<'a, str>,
    #[serde(rename = "editionDate")]
    pub date: Cow<'a, str>,
    #[serde(rename = "editionNumber")]
    pub number: Cow<'a, str>,
}
