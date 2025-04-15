// In src/parser/mod.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod json;

#[derive(Debug, Serialize, Deserialize)]
pub struct SimulatorConfig {
    pub services: HashMap<String, ServiceConfig>,
    pub load: Option<LoadConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub ip: String,
    pub port: String,
    pub methods: HashMap<String, MethodConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MethodConfig {
    pub calls: Vec<Vec<String>>,
    pub latency_distribution: LatencyDistribution,
    pub error_rate: Option<ErrorRate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LatencyDistribution {
    #[serde(rename = "type")]
    pub distribution_type: String,
    pub parameters: HashMap<String, f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorRate {
    #[serde(rename = "type")]
    pub error_type: String,
    pub value: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadConfig {
    pub entry_points: Vec<EntryPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryPoint {
    pub service: String,
    pub method: String,
    pub requests_per_second: u32,
}