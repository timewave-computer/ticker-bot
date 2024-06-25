use localic_std::transactions::ChainRequestBuilder;
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::Value;

use super::{constants::LOGS_PATH, file_system::read_logs_file};

#[derive(Serialize)]
pub struct ValidatorSetEntry {
    pub address: String,
    pub voting_power: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct ValidatorsJson {
    pub validators: Vec<ValidatorSetEntry>,
}

pub fn query_block_height(chain_id: String) -> u64 {
    let logs = read_logs_file(LOGS_PATH).unwrap();
    for chain in logs.chains {
        if chain.chain_id == chain_id {
            let mut url = chain.rpc_address;
            url.push_str("/block");
            let client = Client::new();
            let response = client.post(&url).send().unwrap();
            let json_response = response.json::<Value>().unwrap();
            return json_response["result"]["block"]["header"]["height"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();
        }
    }
    0
}

pub fn query_validator_set(chain: &ChainRequestBuilder) -> Vec<ValidatorSetEntry> {
    let query_valset_cmd = "tendermint-validator-set 100 --output=json".to_string();

    let valset_resp = chain.q(&query_valset_cmd, false);

    let mut val_set_entries: Vec<ValidatorSetEntry> = Vec::new();

    for entry in valset_resp["validators"].as_array().unwrap() {
        let address = entry["address"].as_str().unwrap();
        let voting_power = entry["voting_power"].as_str().unwrap();

        val_set_entries.push(ValidatorSetEntry {
            name: format!("val{}", val_set_entries.len() + 1),
            address: address.to_string(),
            voting_power: voting_power.to_string(),
        });
    }
    val_set_entries
}

pub fn get_keyring_accounts(rb: &ChainRequestBuilder) {
    let accounts = rb.binary("keys list --keyring-backend=test", false);

    let addrs = accounts["addresses"].as_array();
    addrs.map_or_else(
        || {
            println!("No accounts found.");
        },
        |addrs| {
            for acc in addrs.iter() {
                let name = acc["name"].as_str().unwrap_or_default();
                let address = acc["address"].as_str().unwrap_or_default();
                println!("Key '{name}': {address}");
            }
        },
    );
}
