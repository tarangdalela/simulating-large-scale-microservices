// src/main.rs

//extern crate yaml_rust;


use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf, process::Command, str::FromStr};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};
use tonic::transport::{Endpoint, Channel};
use tonic::{Request, Status};
use yaml_rust::{YamlEmitter, YamlLoader, Yaml};
use yaml_rust::yaml::Hash;


// Assuming your generated gRPC stubs are in a module named 'service_stubs'
pub mod service_stubs {
    tonic::include_proto!("service"); 
}
use service_stubs::service_client::ServiceClient;

// Assuming your config structs are defined as provided previously
#[derive(Deserialize, Debug)]
pub struct Config {
    pub services: HashMap<String, ServiceConfig>,
    pub load: LoadConfig,
}

#[derive(Deserialize, Debug, serde::Serialize)] // Added serde::Serialize
pub struct ServiceConfig {
    #[serde(rename = "container_port")]
    pub container_port: u16,
    #[serde(rename = "methods")]
    pub methods: HashMap<String, MethodConfig>,
}


#[derive(Deserialize, Debug, serde::Serialize, Clone)] // Added serde::Serialize and Clone
pub struct MethodConfig {
    #[serde(rename = "calls")]
    pub calls: Vec<Vec<String>>, // Vec<String> and Vec<Vec<String>> derive Clone if String does
    #[serde(rename = "latency_distribution")]
    pub latency_distribution: LatencyDistribution, // LatencyDistribution must derive Clone
    #[serde(rename = "error_rate")]
    pub error_rate: ErrorRate, // ErrorRate must derive Clone
}



#[derive(Deserialize, Debug, serde::Serialize, Clone)] // Added serde::Serialize and Clone
pub struct LatencyDistribution {
    #[serde(rename = "type")]
    pub distribution_type: String,
    pub parameters: HashMap<String, f64>, // HashMap derives Clone if K and V derive Clone
}

#[derive(Deserialize, Debug, serde::Serialize, Clone)] // Added serde::Serialize and Clone
pub struct ErrorRate {
    #[serde(rename = "type")] // This maps the YAML key 'type' to the Rust field 'distribution_type'
    pub distribution_type: String,
    pub parameters: HashMap<String, f64>, // This maps the YAML key 'parameters' to a HashMap
}



#[derive(Deserialize, Debug)]
pub struct LoadConfig {
    #[serde(rename = "entry_points")]
    pub entry_points: Vec<EntryPoint>,
}

#[derive(Deserialize, Debug)]
pub struct EntryPoint {
    #[serde(rename = "service")]
    pub service: String,
    #[serde(rename = "method")]
    pub method: String,
    #[serde(rename = "requests_per_second")]
    pub requests_per_second: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // reading and validating JSON config
    let config = read_and_validate_config("test_config.yaml")?;

    // assign ports
    let port_assignments = assign_ports(&config.services)?;
    info!("Port assignments: {:?}", port_assignments);

    // Generate service-specific config files
    generate_service_configs(&config)?;

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
pub fn read_and_validate_config(file_path: &str) -> Result<Config> {
    info!("Reading configuration from: {}", file_path);
    let contents = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read config file: {}", file_path))?;

    info!("Parsing YAML configuration."); // Updated log message
    let config: Config = serde_yaml::from_str(&contents) // Use serde_yaml::from_str
        .with_context(|| "Failed to parse YAML configuration")?; // Updated error context

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
}

pub fn assign_ports(services: &HashMap<String, ServiceConfig>) -> Result<HashMap<String, u16>> {
    info!("Assigning ports to services.");
    let mut port_assignments = HashMap::new();
    let mut available_ports = (50051..60000).collect::<Vec<u16>>(); // Define a range of ports

    for service_name in services.keys() {
        if let Some(index) = available_ports.pop() {
            port_assignments.insert(service_name.clone(), index);
            debug!("Assigned port {} to service {}", index, service_name);
        } else {
            error!("Ran out of available ports.");
            return Err(anyhow::anyhow!("Ran out of available ports."));
        }
    }

    info!("Port assignment complete: {:?}", port_assignments);
    Ok(port_assignments)
}


pub fn generate_docker_compose_old(
    config: &Config,
    ports: &HashMap<String, u16>,
) -> Result<()> {
    info!("Generating docker-compose.yml file.");
    // Initialize doc as a Hash directly first
    let mut doc_hash = Hash::new();

    // Get a mutable reference to the hash to insert into
    // Now insert into the doc_hash
    doc_hash.insert(Yaml::String("version".into()), Yaml::String("3".into()));

    let mut services = Hash::new();
    for (service_name, service_config) in &config.services {
        let mut service_def = Hash::new();

        let mut build_def = Hash::new();
        build_def.insert(Yaml::String("context".into()), Yaml::String("../generic-service".into()));
        build_def.insert(Yaml::String("dockerfile".into()), Yaml::String("Dockerfile.service".into()));
        // Pass the container_port as a build argument to the Dockerfile.service
        let mut build_args = Hash::new();
        build_args.insert(Yaml::String("SERVICE_CONTAINER_PORT".into()), Yaml::String(service_config.container_port.to_string()));
        build_def.insert(Yaml::String("args".into()), Yaml::Hash(build_args));


        service_def.insert(Yaml::String("build".into()), Yaml::Hash(build_def));
        service_def.insert(Yaml::String("container_name".into()), Yaml::String(service_name.clone().into()));


        if let Some(&host_port) = ports.get(service_name) {
            // Map the assigned host port to the container's internal gRPC port from config
            let ports_mapping = format!("{}:{}", host_port, service_config.container_port);
            service_def.insert(Yaml::String("ports".into()), Yaml::Array(vec![Yaml::String(ports_mapping)]));
        } else {
             error!("Port not assigned for service: {}", service_name);
             return Err(anyhow::anyhow!("Port not assigned for service: {}", service_name));
        }


        let mut environment = Hash::new();
         // Add environment variables for each method's configuration -- 
        for (method_name, method_config) in &service_config.methods {
             let env_var_name = format!("METHOD_{}", method_name.to_uppercase());
             // This now works because MethodConfig derives Serialize
             match serde_json::to_string(method_config) {
                 Ok(method_json) => {
                      environment.insert(Yaml::String(env_var_name.into()), Yaml::String(method_json));
                 },
                 Err(e) => {
                      error!("Failed to serialize method {} for service {}: {}", method_name, service_name, e);
                      return Err(anyhow::anyhow!("Failed to serialize method {} for service {}: {}", method_name, service_name, e));
                 }
             }
        }
        // Add the SERVICE_PORT environment variable (matches the container port from config)
        environment.insert(Yaml::String("SERVICE_PORT".into()), Yaml::String(service_config.container_port.to_string()));

        // Add addresses of services this service calls (using environment variables)
         for method_config in service_config.methods.values() {
             for call_group in &method_config.calls {
                 for call in call_group {
                     if let Some((target_service, _)) = call.split_once('.') {
                         // Only add address if calling a different service
                         if target_service != service_name {
                              if let Some(target_service_config) = config.services.get(target_service) {
                                  let env_var_name = format!("{}_ADDRESS", target_service.to_uppercase());
                                   // Use the target service name and its container port from config
                                  environment.insert(
                                      Yaml::String(env_var_name),
                                      Yaml::String(format!("{}:{}", target_service, target_service_config.container_port)),
                                  );
                              } else {
                                  // This case should ideally be caught by earlier validation or dependency analysis
                                   error!("Could not find configuration for target service {} called by {}", target_service, service_name);
                                   return Err(anyhow::anyhow!("Could not find configuration for target service {} called by {}", target_service, service_name));
                              }
                         }
                     }
                 }
             }
         }


        service_def.insert(Yaml::String("environment".into()), Yaml::Hash(environment));

        // Add networks (using 'microservice_net' as in the example)
        service_def.insert(Yaml::String("networks".into()), Yaml::Array(vec![Yaml::String("microservice_net".into())]));

        // Add depends_on (you need to determine dependencies from the config)
        // let mut dependencies: Vec<Yaml> = Vec::new();
        //  for method_config in service_config.methods.values() {
        //      for call_group in &method_config.calls {
        //          for call in call_group {
        //              if let Some((target_service, _)) = call.split_once('.') {
        //                  // Add dependency only if calling a different service
        //                  if target_service != service_name {
        //                       dependencies.push(Yaml::String(target_service.into()));
        //                  }
        //              }
        //          }
        //      }
        //  }
         // Remove duplicate dependencies
        //  dependencies.sort();
        //  dependencies.dedup();

        // if !dependencies.is_empty() {
        //      service_def.insert(Yaml::String("depends_on".into()), Yaml::Array(dependencies));
        // } else {
        //      // If there are no dependencies, omit depends_on or set to null
        //       service_def.insert(Yaml::String("depends_on".into()), Yaml::Null);
        // }


        services.insert(Yaml::String(service_name.clone()), Yaml::Hash(service_def));
    }

    // Insert the services hash into the top-level doc_hash
    doc_hash.insert(Yaml::String("services".into()), Yaml::Hash(services));

    // Add the networks definition at the top level
    let mut networks_def = Hash::new();
    let mut microservice_net_def = Hash::new();
    microservice_net_def.insert(Yaml::String("driver".into()), Yaml::String("bridge".into()));
    networks_def.insert(Yaml::String("microservice_net".into()), Yaml::Hash(microservice_net_def));
    // Insert the networks definition into the top-level doc_hash
    doc_hash.insert(Yaml::String("networks".into()), Yaml::Hash(networks_def));

    // Now create the final Yaml document from the hash
    let doc = Yaml::Hash(doc_hash);


    // Create a String buffer to write the YAML into
    let mut output_string = String::new();
    // Create the emitter with a mutable reference to the string buffer
    let mut emitter = YamlEmitter::new(&mut output_string);
    // Dump the YAML structure into the string buffer
    emitter.dump(&doc).unwrap();
    // The YAML output is now in output_string


    let mut compose_path = PathBuf::from(".");
    compose_path.push("docker-compose.yml");

    // Write the output string to the file
    fs::write(&compose_path, output_string)
        .with_context(|| format!("Failed to write docker-compose.yml file to {:?}", compose_path))?;

    info!("docker-compose.yml file generated successfully.");

    Ok(())
}

// New function to generate individual config files for each service
pub fn generate_service_configs(config: &Config) -> Result<()> {
    info!("Generating service-specific configuration files.");
    let config_dir = PathBuf::from("./service_configs"); // Directory to store individual configs

    // Create the config directory if it doesn't exist
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create directory: {:?}", config_dir))?;

    for (service_name, service_config) in &config.services {
        // Create a stripped-down config just for this service instance
        let service_specific_config = ServiceConfig {
            container_port: service_config.container_port, // Although this might not be needed if read from ENV
            methods: service_config.methods.clone(), // Clone methods
            // Networks and depends_on are handled by docker-compose
        };

        // Define the path for the service-specific config file
        let mut service_config_path = config_dir.clone();
        service_config_path.push(format!("{}_config.json", service_name)); // Using JSON for service configs as an example

        // Serialize the service-specific config to JSON (or YAML, consistent with generic service parser)
        let config_json = serde_json::to_string_pretty(&service_specific_config)
            .with_context(|| format!("Failed to serialize config for service: {}", service_name))?;

        // Write the config to the file
        fs::write(&service_config_path, config_json)
            .with_context(|| format!("Failed to write config file for service {}: {:?}", service_name, service_config_path))?;

        info!("Generated config file for service {}: {:?}", service_name, service_config_path);
    }

    info!("Service-specific configuration file generation complete.");
    Ok(())
}


pub fn generate_docker_compose(
    config: &Config,
    ports: &HashMap<String, u16>,
) -> Result<()> {
    info!("Generating docker-compose.yml file.");
    let mut doc_hash = Hash::new();

    doc_hash.insert(Yaml::String("version".into()), Yaml::String("3".into()));

    let mut services = Hash::new();
    for (service_name, service_config) in &config.services {
        let mut service_def = Hash::new();

        let mut build_def = Hash::new();
        build_def.insert(Yaml::String("context".into()), Yaml::String("../generic-service".into()));
        build_def.insert(Yaml::String("dockerfile".into()), Yaml::String("Dockerfile.service".into()));
        // Pass the container_port as a build argument (still useful for EXPOSE in Dockerfile)
        let mut build_args = Hash::new();
        build_args.insert(Yaml::String("SERVICE_CONTAINER_PORT".into()), Yaml::String(service_config.container_port.to_string()));
        build_def.insert(Yaml::String("args".into()), Yaml::Hash(build_args));

        service_def.insert(Yaml::String("build".into()), Yaml::Hash(build_def));
        service_def.insert(Yaml::String("container_name".into()), Yaml::String(service_name.clone().into()));

        if let Some(&host_port) = ports.get(service_name) {
            let ports_mapping = format!("{}:{}", host_port, service_config.container_port);
            service_def.insert(Yaml::String("ports".into()), Yaml::Array(vec![Yaml::String(ports_mapping)]));
        } else {
             error!("Port not assigned for service: {}", service_name);
             return Err(anyhow::anyhow!("Port not assigned for service: {}", service_name));
        }

        let mut environment = Hash::new();
        // Add the SERVICE_NAME environment variable
        environment.insert(Yaml::String("SERVICE_NAME".into()), Yaml::String(service_name.clone().into()));

        // Add the SERVICE_PORT environment variable
        environment.insert(Yaml::String("SERVICE_PORT".into()), Yaml::String(service_config.container_port.to_string()));

        // Define the path where the config file will be mounted INSIDE the container
        let container_config_path = "/app/config.json"; // Example path inside the container
        environment.insert(Yaml::String("CONFIG_PATH".into()), Yaml::String(container_config_path.into()));


        service_def.insert(Yaml::String("environment".into()), Yaml::Hash(environment));

        // Configure volumes to mount the service-specific config file
        let mut volumes: Vec<Yaml> = Vec::new();
        // Path on the host: ./service_configs/<service_name>_config.json
        let host_config_path = format!("./service_configs/{}_config.json", service_name);
        // Mount point inside the container: /app/config.json (matches CONFIG_PATH)
        let volume_mapping = format!("{}:{}", host_config_path, container_config_path);
        volumes.push(Yaml::String(volume_mapping.into()));

        service_def.insert(Yaml::String("volumes".into()), Yaml::Array(volumes));


        // Add networks (using 'microservice_net' as in the example)
        service_def.insert(Yaml::String("networks".into()), Yaml::Array(vec![Yaml::String("microservice_net".into())]));

        // depends_on logic can be adjusted or removed based on whether Docker Compose startup order is critical
        // Based on previous errors and the new config method, removing automatic depends_on from calls might be necessary
        // or implementing more sophisticated dependency analysis.
        // Keeping it commented out for now as per previous discussion.
        /*
        let mut dependencies: Vec<Yaml> = Vec::new();
         // ... dependency logic ...
        if !dependencies.is_empty() {
             service_def.insert(Yaml::String("depends_on".into()), Yaml::Array(dependencies));
        } else {
              service_def.insert(Yaml::String("depends_on".into()), Yaml::Null);
        }
        */

        services.insert(Yaml::String(service_name.clone()), Yaml::Hash(service_def));
    }

    doc_hash.insert(Yaml::String("services".into()), Yaml::Hash(services));

    // Add the networks definition at the top level
    let mut networks_def = Hash::new();
    let mut microservice_net_def = Hash::new();
    microservice_net_def.insert(Yaml::String("driver".into()), Yaml::String("bridge".into()));
    networks_def.insert(Yaml::String("microservice_net".into()), Yaml::Hash(microservice_net_def));
    doc_hash.insert(Yaml::String("networks".into()), Yaml::Hash(networks_def));

    let doc = Yaml::Hash(doc_hash);

    let mut output_string = String::new();
    let mut emitter = YamlEmitter::new(&mut output_string);
    emitter.dump(&doc).unwrap();

    let mut compose_path = PathBuf::from(".");
    compose_path.push("docker-compose.yml");

    fs::write(&compose_path, output_string)
        .with_context(|| format!("Failed to write docker-compose.yml file to {:?}", compose_path))?;

    info!("docker-compose.yml file generated successfully.");

    Ok(())
}


fn run_docker_compose() -> Result<()> { 
    info!("Starting Docker Compose.");
    let output = Command::new("docker-compose")
        .arg("up")
        .arg("-d")
        .output()
        .with_context(|| "Failed to execute 'docker-compojse up -d'")?;

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
                // Endpoint::from_str requires the FromStr trait to be in scope
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
