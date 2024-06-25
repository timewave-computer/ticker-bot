use anyhow::Error;
use serde::{Deserialize, Serialize};

const CONFIG_LOCATION: &str = "./config.toml";

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub contract: Vec<Contract>,
    pub chain: Vec<Chain>,
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ContractType {
    StrideLiquidStaker,
    OsmoLiquidPooler,
    NativeSplitter,
    IbcForwarder,
    AstroportLiquidPooler,
    InterchainRouter,
    SwapHolder,
    TwoPartyPolHolder,
    RemoteChainSplitter,
    NativeRouter,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Contract {
    #[serde(rename = "type")]
    pub ctype: ContractType,
    pub chain_prefix: String,
    pub address: String,
}

#[derive(Deserialize, Serialize)]
pub struct Chain {
    pub chain_prefix: String,
    pub base_denom: String,
    pub endpoint: String,
}

pub fn load_config() -> Result<Config, Error> {
    let content = std::fs::read_to_string(CONFIG_LOCATION)?;
    Ok(toml::from_str(&content)?)
}
