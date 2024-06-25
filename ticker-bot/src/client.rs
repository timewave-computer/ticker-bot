use std::collections::HashMap;

use anyhow::Error;
use cosmos_grpc_client::GrpcClient;

use crate::config::Config;

pub async fn setup_clients(config: &Config) -> Result<HashMap<String, GrpcClient>, Error> {
    let mut clients = HashMap::new();
    for chain in &config.chain {
        let client = GrpcClient::new(&chain.endpoint).await?;
        clients.insert(chain.chain_prefix.clone(), client);
    }
    Ok(clients)
}
