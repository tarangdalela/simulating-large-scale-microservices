use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::SimulatorConfig;

/// Parse a JSON file into a SimulatorConfig
pub fn parse_json_file(path: &Path) -> Result<SimulatorConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read JSON file: {}", path.display()))?;
    
    parse_json_str(&content)
}

/// Parse a JSON string into a SimulatorConfig
pub fn parse_json_str(content: &str) -> Result<SimulatorConfig> {
    let config: SimulatorConfig = serde_json::from_str(content)
        .context("Failed to parse JSON content")?;
    
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_valid_json() {
        let json = r#"
        {
            "services": {
                "ServiceA": {
                    "ip": "127.0.0.1",
                    "port": "8000",
                    "methods": {
                        "ProcessOrder": {
                            "calls": [["ServiceB.ValidatePayment"]],
                            "latency_distribution": {
                                "type": "Normal",
                                "parameters": {
                                    "mean": 150,
                                    "stdev": 25
                                }
                            },
                            "error_rate": {
                                "type": "percentage",
                                "value": 0.02
                            }
                        }
                    }
                },
                "ServiceB": {
                    "ip": "127.0.0.1",
                    "port": "8001",
                    "methods": {
                        "ValidatePayment": {
                            "calls": [],
                            "latency_distribution": {
                                "type": "Normal",
                                "parameters": {
                                    "mean": 80,
                                    "stdev": 10
                                }
                            },
                            "error_rate": {
                                "type": "percentage",
                                "value": 0.05
                            }
                        }
                    }
                }
            },
            "load": {
                "entry_points": [
                    {
                        "service": "ServiceA",
                        "method": "ProcessOrder",
                        "requests_per_second": 10
                    }
                ]
            }
        }
        "#;
        
        let result = parse_json_str(json);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.services.len(), 2);
        assert!(config.services.contains_key("ServiceA"));
        assert!(config.services.contains_key("ServiceB"));
        
        let service_a = &config.services["ServiceA"];
        assert_eq!(service_a.ip, "127.0.0.1");
        assert_eq!(service_a.port, "8000");
        assert_eq!(service_a.methods.len(), 1);
        
        let method = &service_a.methods["ProcessOrder"];
        assert_eq!(method.calls.len(), 1);
        assert_eq!(method.calls[0][0], "ServiceB.ValidatePayment");
        
        let load = config.load.as_ref().unwrap();
        assert_eq!(load.entry_points.len(), 1);
        assert_eq!(load.entry_points[0].service, "ServiceA");
        assert_eq!(load.entry_points[0].method, "ProcessOrder");
        assert_eq!(load.entry_points[0].requests_per_second, 10);
    }
    
    #[test]
    fn test_parse_invalid_json() {
        let invalid_json = r#"
        {
            "Services": {
                "ServiceA": {
                    "ip": "127.0.0.1",
                    "port": "8000",
                    "Methods": {
                        // Missing closing brace
                        "ProcessOrder": {
                            "Calls": [["ServiceB.ValidatePayment"]],
                            "LatencyDistribution": {
                                "type": "Normal",
                                "parameters": {
                                    "mean": 150,
                                    "stdev": 25
                                }
                            }
                    }
                }
            }
        }
        "#;
        
        let result = parse_json_str(invalid_json);
        assert!(result.is_err());
    }
}