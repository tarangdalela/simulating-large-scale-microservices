use anyhow::Result;
use serde_yaml;
use std::collections::{HashMap, HashSet};

use crate::parser::SimulatorConfig;

// Docker Compose data structures
#[derive(Debug, serde::Serialize)]
struct DockerCompose {
    version: String,
    services: HashMap<String, DockerService>,
    networks: HashMap<String, DockerNetwork>,
}

#[derive(Debug, serde::Serialize)]
struct DockerService {
    image: String,
    ports: Vec<String>,
    environment: HashMap<String, String>,
    networks: Vec<String>,
    depends_on: Option<Vec<String>>,
}

#[derive(Debug, serde::Serialize)]
struct DockerNetwork {
    driver: String,
}

pub fn generate_docker_compose_yaml(config: &SimulatorConfig) -> Result<String> {
    let mut docker_services = HashMap::new();
    let mut docker_networks = HashMap::new();
    
    // Create a single shared network for all services
    docker_networks.insert("microservice_net".to_string(), DockerNetwork {
        driver: "bridge".to_string(),
    });
    
    // Create configuration for each service
    for (service_name, service_config) in &config.services {
        // Determine dependencies
        let mut dependencies = HashSet::new();
        for method in service_config.methods.values() {
            for call_sequence in &method.calls {
                for call in call_sequence {
                    let called_service = call.split('.').next().unwrap();
                    dependencies.insert(called_service.to_string());
                }
            }
        }
        
        // Remove self-dependencies
        dependencies.remove(service_name);
        
        let depends_on = if dependencies.is_empty() {
            None
        } else {
            Some(dependencies.into_iter().collect())
        };
        
        // Create environment variables for service configuration
        let mut env_vars = HashMap::new();
        
        // Add method configurations
        for (method_name, method) in &service_config.methods {
            // Serialize method config to JSON, then store as env var
            let method_config_json = serde_json::to_string(method)?;
            env_vars.insert(
                format!("METHOD_{}", method_name.to_uppercase()),
                method_config_json,
            );
        }
        
        // Add port configuration
        env_vars.insert("SERVICE_PORT".to_string(), service_config.port.clone());
        
        // Add Docker service configuration
        docker_services.insert(service_name.clone(), DockerService {
            image: "microservice-simulator:latest".to_string(), // Assuming a common image
            ports: vec![format!("{}:{}", service_config.port, service_config.port)],
            environment: env_vars,
            networks: vec!["microservice_net".to_string()],
            depends_on,
        });
    }
    
    // Create the final Docker Compose structure
    let docker_compose = DockerCompose {
        version: "3".to_string(),
        services: docker_services,
        networks: docker_networks,
    };
    
    // Serialize to YAML
    let yaml = serde_yaml::to_string(&docker_compose)?;
    Ok(yaml)
}