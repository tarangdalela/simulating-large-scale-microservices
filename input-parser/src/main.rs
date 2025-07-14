use anyhow::Result;
use client::cli::CliOptions;
use tokio;

mod client;
mod server;
mod parser;
mod validator;
mod generator;

// Include the generated proto code
pub mod proto {
    tonic::include_proto!("sim");
}

async fn run_from_input(opts: &CliOptions) -> Result<()> {
        // Parse JSON file
        let config = parser::json::parse_json_file(&opts.input)?;
        
        // Validate config
        validator::validate_config(&config)?;
        
        // Generate YAML
        let yaml_str = generator::yaml::generate_simulator_yaml(&config)?;
        
        // Output to stdout or send to orchestrator
        if opts.stdout {
            println!("{}", yaml_str);
        } else {
            // Send to orchestrator
            let simulation_id = client::grpc::submit_config_to_orchestrator(&opts.orchestrator, yaml_str).await?;
            println!("Configuration submitted successfully. Simulation ID: {}", simulation_id);
        }

    Ok(())
}

async fn run_as_server(opts: &CliOptions) -> Result<()> {
        // Start servers for receiving input
        let http_port = 8080;
        let grpc_port = 50052;
        
        // Run both servers concurrently
        let orchestrator_addr = opts.orchestrator.clone();
        let http_handle = tokio::spawn(async move {
            server::http::start_http_server(http_port, orchestrator_addr).await
        });
        
        let orchestrator_addr = opts.orchestrator.clone();
        let grpc_handle = tokio::spawn(async move {
            server::grpc::start_grpc_server(grpc_port, orchestrator_addr).await
        });
        
        println!("Input parser service started:");
        println!("  - HTTP server running on port {}", http_port);
        println!("  - gRPC server running on port {}", grpc_port);
        println!("  - Orchestrator service address: {}", opts.orchestrator);
        
        // Wait for both servers
        tokio::try_join!(
            async { http_handle.await.unwrap() },
            async { grpc_handle.await.unwrap() }
        )?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let opts = client::cli::parse_cli_args();
    
    // If input file is provided, process it directly
    if opts.input.exists() {
        run_from_input(&opts).await?;
    } else {
        run_as_server(&opts).await?;
    }
    
    Ok(())
}
