fn main() {
    tonic_prost_build::configure()
        .out_dir("./src")
        .build_client(true)
        .build_server(true)
        .build_transport(true)
        .type_attribute(".", "#[derive(serde::Serialize,serde::Deserialize)]")
        .extern_path(".google.protobuf.Any", "::prost_wkt_types::Any")
        .extern_path(".google.protobuf.Timestamp", "::prost_wkt_types::Timestamp")
        .extern_path(".google.protobuf.Value", "::prost_wkt_types::Value")
        .extern_path(".google.protobuf.FieldMask", "::prost_wkt_types::FieldMask")
        .extern_path(".mend_protos.common.v1", "crate::mend::common_v1")
        .compile_protos(&["proto/ctfjx.proto"], &["proto"])
        .expect("proto to compile");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=proto");
}
