use futures::future;
use rand_distr::{Bernoulli, Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use tonic::{transport::Server, Request, Response, Status};

pub mod service_stubs {
    tonic::include_proto!("service");
}

use service_stubs::service_server::{Service, ServiceServer};
use service_stubs::{ServiceRequest, ServiceResponse};

#[derive(Serialize, Deserialize)]
struct ServiceConfigFromJSON {
    ip: String,
    port: String,
    methods: HashMap<String, MethodConfigFromJSON>,
}
#[derive(Serialize, Deserialize)]
struct MethodConfigFromJSON {
    calls: Option<Vec<Vec<String>>>,
    latency_distribution: DistributionConfigFromJSON,
    error_rate: DistributionConfigFromJSON,
}

#[derive(Serialize, Deserialize)]
struct DistributionConfigFromJSON {
    distribution_type: String,
    parameters: HashMap<String, f64>,
}

trait DistributionSimulator<T> {
    fn simulate(&self) -> T;
}

struct NormalDistribution {
    distribution: rand_distr::Normal<f64>,
}

impl DistributionSimulator<f64> for NormalDistribution {
    fn simulate(&self) -> f64 {
        let mut rng = rand::rng();
        self.distribution.sample(&mut rng)
    }
}

struct BernoulliDistribution {
    distribution: rand_distr::Bernoulli,
}

impl DistributionSimulator<bool> for BernoulliDistribution {
    fn simulate(&self) -> bool {
        let mut rng = rand::rng();
        self.distribution.sample(&mut rng)
    }
}

pub struct GenericService {
    config: HashMap<String, ServiceConfigFromJSON>,
    service_name: String,
}

impl GenericService {
    pub async fn new() -> Self {
        let config_path_str =
            env::var("CONFIG_PATH").unwrap_or_else(|_| "config/config.json".to_string());
        let config_path = Path::new(&config_path_str);
        let config: HashMap<String, ServiceConfigFromJSON> = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("Failed to read config file"),
        )
        .expect("Failed to parse config file");
        let service_name = env::var("SERVICE_NAME").expect("Failed to get SERVICE_NAME");
        config
            .get(&service_name)
            .expect("Own service not found in config");
        GenericService {
            config,
            service_name,
        }
    }

    pub async fn call_service(&self, service_name: &str, method_name: &str) {
        println!(
            "Calling service: {} with method: {}",
            service_name, method_name
        );

        let service_config = self.config.get(service_name).expect("service not found");
        let service_ip = service_config.ip.clone();
        let service_port = service_config.port.clone();

        let service_url = format!("http://{}:{}", service_ip, service_port);
        println!("{}", service_url);
        let mut client = service_stubs::service_client::ServiceClient::connect(service_url)
            .await
            .expect("Failed to connect to service");
        let request = tonic::Request::new(ServiceRequest {
            method_name: method_name.to_string(),
        });

        let response = client.get_data(request).await;
        match response {
            Ok(res) => println!("Response: {:?}", res),
            Err(e) => eprintln!("Error calling service: {:?}", e),
        }
    }
}

#[tonic::async_trait]
impl Service for GenericService {
    async fn get_data(
        &self,
        request: Request<ServiceRequest>,
    ) -> Result<Response<ServiceResponse>, Status> {
        let method_name = request.into_inner().method_name;
        println!("Received request for method: {}", method_name);
        let service_config = self
            .config
            .get(&self.service_name)
            .expect("Own service not found in config"); // should be unreachable
        let method_cnf = service_config
            .methods
            .get(&method_name)
            .expect("Method not found in config");
        match &method_cnf.calls {
            Some(calls) => {
                for call_row in calls {
                    let mut futures = Vec::new();
                    for call in call_row {
                        let mut call = call.split(".");
                        let service_to_call = call.next().expect("Failed to get call name");
                        let method_to_call = call.next().expect("Failed to get method name");
                        futures.push(self.call_service(service_to_call, method_to_call));
                    }
                    future::join_all(futures).await;
                }
            }
            None => {
                println!("No calls to make");
            }
        }
        Ok(Response::new(ServiceResponse {}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("SERVICE_PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let service = GenericService::new().await;

    println!("🚀 Generic Service listening on {}", addr);

    Server::builder()
        .add_service(ServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
