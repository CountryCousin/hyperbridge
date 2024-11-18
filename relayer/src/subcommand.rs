use std::str::FromStr;

use crate::{cli::create_client_map, config::HyperbridgeConfig, logging};
use anyhow::anyhow;
use ismp::host::StateMachine;
use std::sync::Arc;
use tesseract_primitives::IsmpHost;
use tesseract_substrate::config::{Blake2SubstrateChain, KeccakSubstrateChain};

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
	/// Set consensus state for a client on Hyperbridge
	SetConsensusState(SetConsensusState),
	/// Output the json serialized `CreateConsensusState` Message for a client
	LogConsensusState(SetConsensusState),
}

#[derive(Debug, clap::Parser)]
#[command(
	propagate_version = true,
	args_conflicts_with_subcommands = true,
	subcommand_negates_reqs = true
)]
pub struct SetConsensusState {
	/// State Machine whose consensus state should be generated
	state_machine: String,
}

impl SetConsensusState {
	pub async fn set_consensus_state(&self, config_path: String) -> Result<(), anyhow::Error> {
		logging::setup()?;

		let state_machine = StateMachine::from_str(&self.state_machine)
			.map_err(|_| anyhow!("Failed to deserialize state machine"))?;
		log::info!("🧊 Setting consensus state on {state_machine}");
		let config = HyperbridgeConfig::parse_conf(&config_path).await?;
		let HyperbridgeConfig { hyperbridge: hyperbridge_config, relayer, .. } = config.clone();

		let hyperbridge = hyperbridge_config
			.clone()
			.into_client::<Blake2SubstrateChain, KeccakSubstrateChain>()
			.await?;

		let clients = create_client_map(config).await?;

		let client = clients
			.get(&state_machine)
			.ok_or_else(|| anyhow!("Client for provided state machine was not found"))?;

		let mut consensus_state = client
			.query_initial_consensus_state()
			.await?
			.ok_or_else(|| anyhow!("The state machine provided does not have a consensus state"))?;

		let challenge_period = relayer.unwrap_or_default().challenge_period.unwrap_or_default();
		consensus_state.challenge_periods = consensus_state
			.challenge_periods
			.into_iter()
			.map(|(key, _)| (key, challenge_period))
			.collect();

		hyperbridge.client().create_consensus_state(consensus_state).await?;

		Ok(())
	}

	pub async fn log_consensus_state(&self, config_path: String) -> Result<(), anyhow::Error> {
		// using env_logger because tracing subscriber does not allow the output to be piped
		env_logger::init();
		let state_machine = StateMachine::from_str(&self.state_machine)
			.map_err(|_| anyhow!("Failed to deserialize state machine"))?;

		log::info!("🧊 Fetching consensus state for {state_machine}");
		let config = HyperbridgeConfig::parse_conf(&config_path).await?;

		let hyperbridge = config
			.hyperbridge
			.clone()
			.into_client::<Blake2SubstrateChain, KeccakSubstrateChain>()
			.await?;

		let mut clients = create_client_map(config.clone()).await?;

		clients.insert(hyperbridge.provider().state_machine_id().state_id, Arc::new(hyperbridge));

		let client = clients
			.get(&state_machine)
			.ok_or_else(|| anyhow!("Client for provided state machine was not found"))?;

		let consensus_state = client
			.query_initial_consensus_state()
			.await?
			.ok_or_else(|| anyhow!("The state machine provided does not have a consensus state"))?;

		println!("ConsensusState: {}", hex::encode(&consensus_state.consensus_state));

		Ok(())
	}
}
