use futures::future;
use rand_distr::{Bernoulli, Distribution, Normal};
use serde::{Deserialize, Serialize};
use service_stubs::service_client::ServiceClient;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tonic::transport::Channel;
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

struct ServiceConfig {
    methods: HashMap<String, MethodConfig>,
}

struct MethodConfig {
    calls: Option<Vec<Vec<String>>>,
    latency_distribution: Box<dyn DistributionSimulator<f64>>,
    error_rate: Box<dyn DistributionSimulator<bool>>,
}

#[derive(Serialize, Deserialize)]
struct DistributionConfigFromJSON {
    distribution_type: String,
    parameters: HashMap<String, f64>,
}

trait DistributionSimulator<T>: Send + Sync {
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
    config: ServiceConfig,
    config_json: HashMap<String, ServiceConfigFromJSON>,
    services: Arc<Mutex<HashMap<String, ServiceClient<Channel>>>>,
}

impl GenericService {
    pub async fn new() -> Self {
        let config_path_str =
            env::var("CONFIG_PATH").unwrap_or_else(|_| "config/config.json".to_string());
        let config_path = Path::new(&config_path_str);
        let config_json: HashMap<String, ServiceConfigFromJSON> = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("Failed to read config file"),
        )
        .expect("Failed to parse config file");
        let service_name = env::var("SERVICE_NAME").expect("Failed to get SERVICE_NAME");
        config_json
            .get(&service_name)
            .expect("Own service not found in config");
        let config = ServiceConfig {
            methods: config_json[&service_name]
                .methods
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        MethodConfig {
                            calls: v.calls.clone(),
                            latency_distribution: match v
                                .latency_distribution
                                .distribution_type
                                .as_str()
                            {
                                "normal" => Box::new(NormalDistribution {
                                    distribution: Normal::new(
                                        v.latency_distribution.parameters["mean"],
                                        v.latency_distribution.parameters["stddev"],
                                    )
                                    .unwrap(),
                                }),
                                _ => panic!("Unsupported distribution type"),
                            },
                            error_rate: match v.error_rate.distribution_type.as_str() {
                                "bernoulli" => Box::new(BernoulliDistribution {
                                    distribution: Bernoulli::new(v.error_rate.parameters["p"])
                                        .unwrap(),
                                }),
                                _ => panic!("Unsupported distribution type"),
                            },
                        },
                    )
                })
                .collect(),
        };
        GenericService {
            config,
            config_json,
            services: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn call_service(&self, service_name: &str, method_name: &str) -> bool {
        println!(
            "Calling service: {} with method: {}",
            service_name, method_name
        );

        let service_ip = self.config_json[service_name].ip.clone();
        let service_port = self.config_json[service_name].port.clone();

        let service_url = format!("http://{}:{}", service_ip, service_port);
        println!("{}", service_url);
        let mut services = self.services.lock().await;
        let mut client = if let Some(client) = services.get(&service_url) {
            client.clone()
        } else {
            match ServiceClient::connect(service_url.clone()).await {
                Ok(client) => {
                    services.insert(service_url.clone(), client.clone());
                    client
                }
                Err(e) => {
                    eprintln!("Failed to connect to service: {:?}", e);
                    return false;
                }
            }
        };
        drop(services);
        let request = tonic::Request::new(ServiceRequest {
            method_name: method_name.to_string(),
        });

        let response = client.get_data(request).await;
        match response {
            Ok(res) => {
                println!("Response: {:?}", res);
                true
            }
            Err(e) => {
                eprintln!("Error calling service: {:?}", e);
                false
            }
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
        let method_cnf = self
            .config
            .methods
            .get(&method_name)
            .expect("Method not found in config");
        match &method_cnf.calls {
            Some(calls) => {
                for call_row in calls {
                    println!("hi");
                    let mut succeeded = vec![false; call_row.len()];
                    while succeeded.contains(&false) {
                        println!("ahhh");
                        let mut futures = Vec::new();
                        for (i, call) in call_row.iter().enumerate() {
                            if succeeded[i] {
                                continue;
                            }
                            let mut call = call.split(".");
                            let service_to_call = call.next().expect("Failed to get call name");
                            let method_to_call = call.next().expect("Failed to get method name");
                            futures.push(self.call_service(service_to_call, method_to_call));
                        }
                        let resp = future::join_all(futures).await;
                        let mut j = 0;
                        (0..succeeded.len()).for_each(|i| {
                            if !succeeded[i] {
                                succeeded[i] = resp[j];
                                j += 1;
                            }
                        });
                    }
                }
            }
            None => {
                println!("No calls to make");
            }
        }
        println!("gonna wait");
        // wait latency
        let latency = method_cnf.latency_distribution.simulate();
        sleep(std::time::Duration::from_millis(latency.round() as u64)).await;
        let error_rate = method_cnf.error_rate.simulate();
        if error_rate {
            println!("error rate triggered");
            return Err(Status::internal("Internal error"));
        }

        println!("finished");
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
