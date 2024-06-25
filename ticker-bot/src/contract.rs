use anyhow::Error;
use cosmos_grpc_client::{
    cosmos_sdk_proto::{
        cosmos::{
            bank::v1beta1::QueryAllBalancesRequest, base::v1beta1::Coin,
            tx::v1beta1::BroadcastTxResponse,
        },
        cosmwasm::wasm::v1::{MsgExecuteContract, QuerySmartContractStateRequest},
    },
    BroadcastMode, GrpcClient, ProstMsgNameToAny, Wallet,
};
use log::info;
use serde::{Deserialize, Serialize};

use crate::config::ContractType;

#[derive(PartialEq, Clone)]
pub struct ContractWithData {
    pub ctype: ContractType,
    pub chain_prefix: String,
    pub address: String,
    pub state: Vec<u8>,
    pub balances: Vec<Coin>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Messages {
    ContractState {},
    Tick {},
}

impl ContractWithData {
    pub const fn as_str(&self) -> &'static str {
        match self.ctype {
            ContractType::StrideLiquidStaker => "stride_liquid_staker",
            ContractType::OsmoLiquidPooler => "osmo_liquid_pooler",
            ContractType::NativeSplitter => "native_splitter",
            ContractType::IbcForwarder => "ibc_forwarder",
            ContractType::AstroportLiquidPooler => "astroport_liquid_pooler",
            ContractType::InterchainRouter => "interchain_router",
            ContractType::SwapHolder => "swap_holder",
            ContractType::TwoPartyPolHolder => "two_party_pol_holder",
            ContractType::RemoteChainSplitter => "remote_chain_splitter",
            ContractType::NativeRouter => "native_router",
        }
    }

    pub async fn query_state(&self, client: &mut GrpcClient) -> Result<Vec<u8>, Error> {
        match self.ctype {
            // For these we will only check balances
            ContractType::NativeSplitter
            | ContractType::InterchainRouter
            | ContractType::NativeRouter => {
                return Ok(vec![]);
            }
            _ => {}
        }

        info!(
            "Querying state of contract {} ({})...",
            self.address,
            self.as_str()
        );
        let response = client
            .clients
            .wasm
            .smart_contract_state(QuerySmartContractStateRequest {
                address: self.address.clone(),
                query_data: serde_json::to_string(&Messages::ContractState {})?
                    .as_bytes()
                    .to_vec(),
            })
            .await?
            .into_inner();

        Ok(response.data)
    }

    pub async fn query_balances(&self, client: &mut GrpcClient) -> Result<Vec<Coin>, Error> {
        info!(
            "Querying balance of contract {} ({})...",
            self.address,
            self.as_str()
        );

        let balances = client
            .clients
            .bank
            .all_balances(QueryAllBalancesRequest {
                address: self.address.clone(),
                pagination: None,
            })
            .await?
            .into_inner();

        Ok(balances.balances)
    }

    pub async fn tick(&self, wallet: &mut Wallet) -> Result<BroadcastTxResponse, Error> {
        info!("Ticking contract {} ({})...", self.address, self.as_str());
        let msg = MsgExecuteContract {
            sender: wallet.account_address.clone(),
            contract: self.address.clone(),
            msg: serde_json::to_string(&Messages::Tick {})?
                .as_bytes()
                .to_vec(),
            funds: vec![],
        }
        .build_any();

        let response = wallet
            .broadcast_tx(
                vec![msg],
                // Fee, will be calculated automatically
                None,
                // Memo
                None,
                // Broadcast mode; Block/Sync/Async
                BroadcastMode::Sync,
            )
            .await?;

        Ok(response)
    }
}
