fn main() {
    tonic_build::compile_protos("src/protos/ratelimit.proto").unwrap();
}
