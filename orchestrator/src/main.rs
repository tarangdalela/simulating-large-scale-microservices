// src/main.rs

//extern crate yaml_rust;



use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, fs};
use tokio;
use tracing::{debug, error, info};
use yaml_rust::{YamlEmitter, YamlLoader, Yaml};
use yaml_rust::yaml::Hash; 
use tokio::time::{interval, Duration};
use tracing::{error, info, debug};
use tonic::transport::Endpoint;
use tonic::{Request, Status, transport::Channel};


// importing generated client
use crate::service_stubs::service_client::ServiceClient; 



#[derive(Deserialize)]
struct Config {
    services: HashMap<String, ServiceConfig>,
    load: LoadConfig,
}

#[derive(Deserialize)]
struct ServiceConfig {
    //ip: String, // Consider removing or using later
    port: String, // Orchestrator will assign
    methods: HashMap<String, MethodConfig>,
}

#[derive(Deserialize)]
struct MethodConfig {
    calls: Vec<Vec<String>>,
    latency_distribution: LatencyDistribution,
    error_rate: ErrorRate,
}

#[derive(Deserialize)]
struct LatencyDistribution {
    #[serde(rename = "type")]
    distribution_type: String,
    parameters: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct ErrorRate {
    #[serde(rename = "type")]
    rate_type: String,
    value: f64,
}

#[derive(Deserialize)]
struct LoadConfig {
    entry_points: Vec<EntryPoint>,
}

#[derive(Deserialize)]
struct EntryPoint {
    service: String,
    method: String,
    requests_per_second: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // reading and validating JSON config
    let config = read_and_validate_config("path/to/config.json")?;

    // assign ports
    let port_assignments = assign_ports(&config.services)?;
    info!("Port assignments: {:?}", port_assignments);

    // generate docker-compose.yml
    generate_docker_compose(&config, &port_assignments)?;

    // running Docker Compose
    run_docker_compose()?;

    // creating the load (make sure to start after a short delay to ensure services are up and working)
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    start_load_generation(&config.load, &port_assignments).await?;

    // wait for termination signal (ctrl-c in this case) and then stopping docker compose
    tokio::signal::ctrl_c().await?;
    info!("Received termination signal.");    
    stop_docker_compose()?;

    // collect and report output (TODO)
    info!("Collecting and reporting output...");

    Ok(())
}

// Implement functions for each step:
fn read_and_validate_config(file_path: &str) -> Result<Config> { 
    /* something from gemini for now? as of 4/14 could not find anything in repo */ 
    info!("Reading configuration from: {}", file_path);
    let contents = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read config file: {}", file_path))?;

    info!("Parsing JSON configuration.");
    let config: Config = serde_json::from_str(&contents)
        .with_context(|| "Failed to parse JSON configuration")?;

    info!("Performing basic configuration validation.");
    if config.services.is_empty() {
        error!("No services defined in the configuration.");
        return Err(anyhow::anyhow!("No services defined in the configuration."));
    }

    if config.load.entry_points.is_empty() {
        info!("No load entry points defined in the configuration. Simulation will start but might not generate load.");
    } else {
        for entry_point in &config.load.entry_points {
            if !config.services.contains_key(&entry_point.service) {
                error!(
                    "Load entry point refers to non-existent service: {}",
                    entry_point.service
                );
                return Err(anyhow::anyhow!(
                    "Load entry point refers to non-existent service: {}",
                    entry_point.service
                ));
            } else if let Some(service_config) = config.services.get(&entry_point.service) {
                if !service_config.methods.contains_key(&entry_point.method) {
                    error!(
                        "Load entry point refers to non-existent method '{}' in service '{}'.",
                        entry_point.method, entry_point.service
                    );
                    return Err(anyhow::anyhow!(
                        "Load entry point refers to non-existent method '{}' in service '{}'.",
                        entry_point.method, entry_point.service
                    ));
                }
            }
        }
    }

    // You can add more validation logic here as needed,
    // for example, checking the validity of latency distribution types, etc.

    info!("Configuration read and validated successfully.");
    Ok(config)

} // this is prob what the input parser is doing

fn assign_ports(services: &HashMap<String, ServiceConfig>) -> Result<HashMap<String, u16>> { 
    
    //assigning ports to each service if possible
    let mut port_assignments = HashMap::new();
    let mut available_ports = (50051..60000).collect::<Vec<u16>>();

    // iterating through each service in the hashmap
    for service_name in services.keys() {
        if let Some(index) = available_ports.pop() {
            port_assignments.insert(service_name.clone(), index);
            debug!("Assigning service {} to port {}", service_name, index);
        }  else {
            error!("Ran out of ports to use.");
            return Err(anyhow::anyhow!("Ran out of ports to use."));
        }
    }

    info!("Port assignment complete: {:?}", port_assignments);
    Ok(port_assignments)

}

fn generate_docker_compose(config: &Config, ports: &HashMap<String, u16>) -> Result<()> { 
    info!("generating docker compose file");
    let mut doc = Yaml::Hash(yaml_rust::yaml::Hash::new());

    // setting the version
    doc.insert(Yaml::String("version".into()), Yaml::String("3.8".into()));

    let mut services = yaml_rust::yaml::Hash::new();

    for (service_name, service_config) in &config.services {
        let mut service_def = yaml_rust::yaml::Hash::new();
        service_def.insert(
            Yaml::String("image".into()), 
            Yaml::String("image_name:latest".into())
        ); // need to replace with the actual image name, just temp placeholder

        if let Some(&port) = ports.get(service_name) {
            let ports_mapping = format!("{}:50051", port);
            service_def.insert(Yaml::String("ports".into()), Yaml::Array(vec![Yaml::String(ports_mapping)]));
        }

        let mut environment = yaml_rust::yaml::Hash::new();
        environment.insert(
            Yaml::String("SERVICE_NAME".into()),
            Yaml::String(service_name.clone()),
        );

        // now seralizing and adding methods configuration as an enivornment variable

        match serde_json::to_string(&service_config.methods) {
            Ok(methods_json) => {
                environment.insert(Yaml::String("METHODS".into()), Yaml::String(methods_json));
            } 
            Err(e) => {
                error!("Failed to serialize methods for service {}: {}", service_name, e);
                return Err(anyhow::anyhow!("Failed to serialize methods for service {}: {}"));
            }
        }

        // Add addresses of services this service calls
        if let Some(first_method) = service_config.methods.values().next() { // Just take the first method to iterate through calls
            for call_group in &first_method.calls {
                for call in call_group {
                    if let Some((target_service, _)) = call.split_once('.') {
                        if let Some(&target_port) = ports.get(target_service) {
                            let env_var_name = format!("{}_ADDRESS", target_service.to_uppercase());
                            environment.insert(
                                Yaml::String(env_var_name),
                                Yaml::String(format!("localhost:{}", target_port)), // Assuming all services run on localhost in Docker
                            );
                        } else {
                            error!("Could not find port assignment for service {} called by {}", target_service, service_name);
                            return Err(anyhow::anyhow!("Could not find port assignment for service {} called by {}", target_service, service_name));
                        }
                    }
                }
            }
        }

        service_def.insert(Yaml::String("environment".into()), Yaml::Hash(environment));
        services.insert(Yaml::String(service_name.clone()), Yaml::Hash(service_def));

    }

    doc.insert(Yaml::String("services".into()), Yaml::Hash(services));

    
    let mut emitter = YamlEmitter::new();
    emitter.dump(&doc).unwrap(); // dumping the YAML structure to the emitter
    let output = emitter.emit(); // getting the emitted YAML string

    // constructing the path to docker-compose.yml in the project root
    let mut compose_path = PathBuf::from("."); // curr directory (project root when running with cargo run)
    compose_path.push("docker-compose.yml");

    fs::write(&compose_path, output)
        .with_context(|| format!("Failed to write docker-compose.yml file to {:?}", compose_path))?;

    info!("docker-compose.yml file generated successfully in the project root.");


    Ok(())


}
fn run_docker_compose() -> Result<()> { 
    info!("Starting Docker Compose.");
    let output = Command::new("docker-compose")
        .arg("up")
        .arg("-d")
        .output()
        .with_context(|| "Failed to execute 'docker-compose up -d'")?;

    if output.status.success() {
        info!("Docker Compose started successfully.");
        debug!("Docker Compose output:\n{}", String::from_utf8_lossy(&output.stdout));
        if !output.stderr.is_empty() {
            debug!("Docker Compose stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    } else {
        error!("Failed to start Docker Compose.");
        error!("Stdout:\n{}", String::from_utf8_lossy(&output.stdout));
        error!("Stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        Err(anyhow::anyhow!("Failed to start Docker Compose. Check output for details."))
    }

}

fn stop_docker_compose() -> Result<(), anyhow::Error> { 
   
    info!("Stopping Docker Compose.");
    let output = Command::new("docker-compose")
        .arg("down")
        .output()
        .with_context(|| "Failed to execute 'docker-compose down'")?;

    if output.status.success() {
        info!("Docker Compose stopped successfully.");
        debug!("Docker Compose output:\n{}", String::from_utf8_lossy(&output.stdout));
        if !output.stderr.is_empty() {
            debug!("Docker Compose stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    } else {
        error!("Failed to stop Docker Compose.");
        error!("Stdout:\n{}", String::from_utf8_lossy(&output.stdout));
        error!("Stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        Err(anyhow::anyhow!("Failed to stop Docker Compose. Check output for details."))
    }
}

async fn start_load_generation(load_config: &LoadConfig, ports: &HashMap<String, u16>) -> Result<(), anyhow::Error> { 
    info!("Setting up load generation.");
    // this uses proto definition from generic services project
    for entry_point in &load_config.entry_points {
        let service_name = &entry_point.service;
        let method_name = &entry_point.method;
        let requests_per_second = entry_point.requests_per_second;

        if let Some(&port) = ports.get(service_name) {
            let address = format!("http://localhost:{}", port);
            info!(
                "Starting load generation for {}::{} at {} RPS to {}",
                service_name, method_name, requests_per_second, address
            );

            // Clone necessary data for the async task
            let service_name_clone = service_name.clone();
            let method_name_clone = method_name.clone();

            tokio::spawn(async move {
                let mut interval = interval(Duration::from_micros(
                    (1_000_000.0 / requests_per_second as f64) as u64,
                ));
                let endpoint = Endpoint::from_str(&address).unwrap(); 
                match endpoint.connect().await {
                    Ok(channel) => {
                        let mut client = ServiceClient::new(channel);
                        let mut request_counter: u64 = 0;

                        loop {
                            interval.tick().await;

                            let request = Request::new(crate::service_stubs::ServiceRequest { 
                                method_name: method_name_clone.clone(),
                            });

                            match client.get_data(request).await { 
                                Ok(response) => {
                                    debug!(
                                        "Request to {}::{} successful. Response: {:?}",
                                        service_name_clone, method_name_clone, response
                                    );
                                    request_counter += 1;
                                }
                                Err(status) => {
                                    error!(
                                        "Error sending request to {}::{} : {:?}",
                                        service_name_clone, method_name_clone, status
                                    );
                                }
                            }
                            // You might want to add a condition to stop the load generation eventually
                            // or handle termination signals here.
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to connect to {} at {}: {}",
                            service_name_clone, address, e
                        );
                    }
                }
            });
        } else {
            error!(
                "No port assigned for service: {} in load generation.",
                service_name
            );
        }
    }

    info!("Load generation setup complete.");
    Ok(())
}



// Function to communicate with services to get histograms (to be implemented)
// Function to handle traffic tracking (to be implemented)
