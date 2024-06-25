use std::{io::Write, path::Path};

use localic_std::transactions::ChainRequestBuilder;
use serde::Serialize;
use serde_json::Value;

use super::types::{ChainsVec, Logs};

pub fn read_chains_file(file_path: &str) -> Result<ChainsVec, std::io::Error> {
    // Read the file to a string
    let data = std::fs::read_to_string(file_path)?;

    // Parse the string into the struct
    let chain: ChainsVec = serde_json::from_str(&data)?;

    Ok(chain)
}

pub fn read_logs_file(file_path: &str) -> Result<Logs, std::io::Error> {
    // Read the file to a string
    let data = std::fs::read_to_string(file_path)?;

    // Parse the string into the struct
    let logs: Logs = serde_json::from_str(&data)?;

    Ok(logs)
}

pub fn pretty_print(obj: &Value) {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    obj.serialize(&mut ser).unwrap();
    println!("{}", String::from_utf8(buf).unwrap());
}

pub fn write_json_file(path: &str, data: &str) {
    let path = Path::new(path);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(data.as_bytes()).unwrap();

    println!("file written: {:?}", path);
}

pub fn write_str_to_container_file(rb: &ChainRequestBuilder, container_path: &str, content: &str) {
    // TODO: fix this. perhaps draw inspiration from request_builder upload_file.
    let filewriting = rb.exec(
        &format!("/bin/sh -c echo '{}' > {}", content, container_path),
        true,
    );
    println!("\nwrite str to container file response:\n");
    pretty_print(&filewriting);
}
