use serde::Deserialize;

pub fn read_logs_file(file_path: &str) -> Result<Logs, std::io::Error> {
    // Read the file to a string
    let data = std::fs::read_to_string(file_path)?;

    // Parse the string into the struct
    let logs: Logs = serde_json::from_str(&data)?;

    Ok(logs)
}

#[derive(Deserialize)]
pub struct Logs {
    pub start_time: u64,
    pub chains: Vec<ChainLog>,
    pub ibc_channels: Option<Vec<IbcChannelLog>>,
}

#[derive(Deserialize)]
pub struct ChainLog {
    pub chain_id: String,
    pub chain_name: String,
    pub rpc_address: String,
    pub rest_address: String,
    pub grpc_address: String,
    pub p2p_address: String,
    pub ibc_paths: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct IbcChannelLog {
    pub chain_id: String,
    pub channel: ChannelLog,
}

#[derive(Deserialize)]
pub struct ChannelLog {
    pub channel_id: String,
    pub connection_hops: Vec<String>,
    pub counterparty: CounterpartyLog,
    pub ordering: String,
    pub port_id: String,
    pub state: String,
    pub version: String,
}

#[derive(Deserialize)]
pub struct CounterpartyLog {
    pub channel_id: String,
    pub port_id: String,
}
