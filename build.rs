fn main() {
    let proto_root = "src/protos";
    println!("cargo:rerun-if-changed={}", proto_root);

    protoc_rust_grpc::run(protoc_rust_grpc::Args {
        out_dir: "src/protos",
        includes: &["src/protos"],
        input: &["src/protos/ratelimit.proto"],
        rust_protobuf: true, // also generate protobuf messages, not just services
        ..Default::default()
    }).expect("Failed to compile gRPC definitions!");
}
