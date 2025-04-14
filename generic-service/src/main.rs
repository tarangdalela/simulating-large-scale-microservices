use futures::future;
use serde_json::Value as JValue;
use std::env;
use std::path::Path;
use tonic::{transport::Server, Request, Response, Status};

pub mod service_stubs {
    tonic::include_proto!("service");
}

use service_stubs::service_server::{Service, ServiceServer};
use service_stubs::{ServiceRequest, ServiceResponse};

#[derive(Debug)]
pub struct GenericService {
    config: JValue,
    service_name: String,
}

impl GenericService {
    pub async fn new() -> Self {
        let config_path_str =
            env::var("CONFIG_PATH").unwrap_or_else(|_| "config/config.json".to_string());
        let config_path = Path::new(&config_path_str);
        let config: JValue = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("Failed to read config file"),
        )
        .expect("Failed to parse config file");
        let service_name = env::var("SERVICE_NAME").expect("Failed to get SERVICE_NAME");
        GenericService {
            config: config
                .get("Services")
                .expect("JSON missing services field")
                .clone(),
            service_name,
        }
    }

    pub async fn call_service(&self, service_name: &str, method_name: &str) {
        println!(
            "Calling service: {} with method: {}",
            service_name, method_name
        );

        let service_config = self
            .config
            .get(service_name)
            .expect("Service not found in config");
        let service_ip = service_config
            .get("ip")
            .expect("IP not found in service config")
            .as_str()
            .expect("Failed to convert IP to string");
        let service_port = service_config
            .get("port")
            .expect("Port not found in service config")
            .as_str()
            .expect("Failed to convert port to string");

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
            .expect("Service not found in config");
        let method_cnf = service_config
            .get("Methods")
            .expect("Methods not found in service config")
            .get(&method_name)
            .expect("Method not found in service config");
        let method_calls = method_cnf
            .get("Calls")
            .expect("Calls not found in method config");
        for call_row in method_calls.as_array().unwrap() {
            let mut futures = Vec::new();
            for call in call_row.as_array().unwrap() {
                let call = call
                    .as_str()
                    .expect("Failed to convert call name to string");
                let mut call = call.split(".");
                let service_to_call = call.next().expect("Failed to get call name");
                let method_to_call = call.next().expect("Failed to get method name");
                futures.push(self.call_service(service_to_call, method_to_call));
            }
            future::join_all(futures).await;
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
