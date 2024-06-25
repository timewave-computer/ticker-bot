# Ticker Bot

This bot is an off-chain tool written in Rust that will send `Tick {}` messages to our tickable contracts if their `CONTRACT_STATE` and/or `balances` have changed since the last tick.
To do that, the bot will use the config provided in `config.toml` which contains all the contracts with their info: `type`, `chain_prefix` and `address` that the bot will be tracking. Additionally, we need to also provide the bot with information of the chains and their info: `chain_prefix`, `base_denom` and `endpoint` where those contracts are deployed. The bot will setup the wallets for those chains using the encrypted `mnemonic.key` file provided, which contains the seed phrase encrypted.

Note: the addresses derived from the seed phrase will be used to sign the transactions, so make sure that they have enough funds to cover for the gas fees on each chain.

## Testing

To test the bot, we've prepared a `local-interchaintest` setup that will deploy a local instance of gaia, neutron and stride chains. After that, when we run the tests, it will upload all the necessary astroport and valence contracts and deploy a full `single-party-pol` covenant on the neutron instance and modify the config file of the bot with all the tickable contract addresses corresponding to that deployment and an endpoint to the neutron chain that we deployed locally so that we only need to run it.

To reproduce this:

1. Prior to running the local interchain, we need to have non-ics version of stride docker image. Specifically, we are using the [v9.2.1 tagged version](https://github.com/Stride-Labs/stride/tree/v9.2.1) image. So you can clone this repo and build it using the [heighliner](https://github.com/strangelove-ventures/heighliner#example-cosmos-sdk-chain-development-cycle-build-a-local-repository) tool by strangelove. In the stride repository run:

```bash
heighliner build -c stride --local -t non-ics
```

2. Install interchain test:

```bash
git clone --depth 1 --branch v8.3.0 https://github.com/strangelove-ventures/interchaintest; cd interchaintest; git switch -c v8.3.0
```

```bash
cd local-interchain
```

```bash

make install
```

3. Once interchain test is installed and we have the stride image, we can run our local interchain setup. In the local-interchaintest directory run:

```bash
local-ic start neutron_gaia --api-port 42069
```

This will run the config in the `neutron_gaia.json` file which will deploy a local instance of gaia, neutron and stride chains.

4. After it is deployed, we can run the tests in our local-interchaintest directory by running:

```bash
cargo run --package local-ictest-e2e --bin local-ictest-e2e
```

This step will deploy the convenant, update our ticker bot config and fund the address of the bot to start ticking.

5. Finally we can run the bot by running

```bash
cd ticker-bot && cargo run
```

If you can't see the logs of the bot, set up your RUST_LOG variable to do so:

```bash
export RUST_LOG=debug
```
