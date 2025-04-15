use std::path::PathBuf;
use structopt::StructOpt;
use anyhow::Result;


#[derive(Debug, StructOpt)]
#[structopt(name = "microservice-simulator-parser", about = "Microservice Simulator Input Parser")]
pub struct CliOptions {
    #[structopt(short, long, parse(from_os_str))]
    /// Path to the input JSON file
    pub input: PathBuf,
    
    #[structopt(short, long, default_value = "localhost:50051")]
    /// Address of the orchestrator service
    pub orchestrator: String,
    
    #[structopt(short, long)]
    /// Output the generated YAML to stdout instead of sending to orchestrator
    pub stdout: bool,
}

pub fn parse_cli_args() -> CliOptions {
    CliOptions::from_args()
}