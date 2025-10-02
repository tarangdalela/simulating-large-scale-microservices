# Microservices Evaluator

This project evaluates microservices with various synthetic workloads.

It supports:

1. Arbitrary call graph definitions via a config file
2. Config file-defined service time distribution per service
3. Custom load generation logic for this call graph


## About the project
### Frontend:
This folder contains all the code which helps in developing the application that the user can interact with. Users can create visual call graphs, and custom set different specs of the graph such as the requests per second of a service. 

### Orchestrator
Code can be found in runner. It contains the code needed for the orchestrator to launch the simulation. This part of the project takes in input from the front end in the form of a JSON file, and then uses the Generic Service to create services for each inputted service. 

### Generic Service
Code can be found generic-service folder. This generic service is called for each inputed service, and each service that is spun up has its own Docker image.

## Getting started

Minimal example to run this project:

``` bash
$ cd runner
$ cargo run -- --input ./test_config.json
```
