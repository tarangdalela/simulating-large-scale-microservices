fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_file = "../generic-service/proto/service.proto";
    let proto_file_two = "../input-parser/proto/sim.proto";

    println!("cargo:rerun-if-changed={}", proto_file);

    tonic_build::compile_protos(proto_file)?;
    tonic_build::compile_protos(proto_file_two)?;
    Ok(())
}