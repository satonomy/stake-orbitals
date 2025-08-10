use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
    token::Token,
};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::parcel::AlkaneTransferParcel;
use alkanes_support::{id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse};
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;

use anyhow::{anyhow, Result};
use std::sync::Arc;

const BEEP_BOOP_STAKE_CONTRACT_BLOCK: u128 = 2;
const BEEP_BOOP_STAKE_CONTRACT_TX: u128 = 57751;
const CONTRACT_NAME: &str = "BB";
const CONTRACT_SYMBOL: &str = "🤖";

// Swap rate: 25000 $BB = 1 BEEP BOOP
const SWAP_RATE: u128 = 25000;

// Token supply constants
const MAX_SUPPLY: u128 = 250000000;

// Stake contract opcodes that we need to call
const STAKE_GET_ELIGIBILITY: u128 = 506;
const STAKE_GET_STAKED_HEIGHT: u128 = 507;
const STAKE_GET_STAKED_BY_LP: u128 = 508;
const STAKE_GET_TOTAL_STAKED_BLOCKS: u128 = 510;
const STAKE_GET_TOTAL_STAKED: u128 = 511;
#[derive(Default)]
pub struct Claim(());

impl AlkaneResponder for Claim {}

/// Message types for claim contract interaction
#[derive(MessageDispatch)]
enum ClaimMessage {
    /// Initialize the claim contract
    #[opcode(0)]
    Initialize,

    /// Get the name of the claim contract
    #[opcode(99)]
    #[returns(String)]
    GetName,

    /// Get the symbol of the claim contract
    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    /// Get the total supply of claim contract tokens
    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,

    /// Get the maximum number of tokens that can be minted
    #[opcode(102)]
    #[returns(u128)]
    GetMaxSupply,

    /// Get the current number of minted tokens
    #[opcode(103)]
    #[returns(u128)]
    GetMinted,

    /// Get the value per mint
    #[opcode(104)]
    #[returns(u128)]
    GetValuePerMint,

    /// Get total staked blocks for a specific alkane by calling the stake contract
    #[opcode(300)]
    #[returns(u128)]
    GetTotalStakedByAlkaneId { block: u128, tx: u128 },

    /// Get total claimed amount for a specific alkane ID
    #[opcode(301)]
    #[returns(u128)]
    GetTotalClaimedByAlkaneId { block: u128, tx: u128 },

    /// Calculate total available rewards to claim for a specific alkane ID
    #[opcode(302)]
    #[returns(u128)]
    GetTotalAvailableToClaim { block: u128, tx: u128 },

    /// Claim available rewards for specific alkane IDs
    #[opcode(400)]
    ClaimRewards,

    /// Get total amount claimed across all alkanes
    #[opcode(401)]
    #[returns(u128)]
    GetTotalClaimed,

    /// Get total amount available to claim across all alkanes
    #[opcode(402)]
    #[returns(u128)]
    GetTotalAvailable,

    /// Swap $BB tokens to BEEP BOOPs (25K $BB -> 1 BEEP BOOP)
    #[opcode(501)]
    SwapBBToBeepBoop,

    /// Swap BEEP BOOPs to $BB tokens (1 BEEP BOOP -> 25K $BB)
    #[opcode(502)]
    SwapBeepBoopToBB,

    /// Get current BEEP BOOP supply
    #[opcode(504)]
    #[returns(u128)]
    GetBeepBoopSupply,

    /// Get swap rate (how many $BB for 1 BEEP BOOP)
    #[opcode(506)]
    #[returns(u128)]
    GetSwapRate,

    /// Deposit BEEP BOOP tokens to contract for swapping
    #[opcode(511)]
    DepositBeepBoop,

    /// Get the next swap index (which BEEP BOOP token will be retrieved next)
    #[opcode(512)]
    #[returns(u128)]
    GetNextSwapIndex,

    /// Get the alkane ID of a stored BEEP BOOP token by index
    #[opcode(513)]
    #[returns(String)]
    GetStoredBeepBoopAlkaneId { index: u128 },

    /// Get collection identifier
    #[opcode(998)]
    #[returns(String)]
    GetCollectionIdentifier,
}

impl Token for Claim {
    fn name(&self) -> String {
        String::from(CONTRACT_NAME)
    }

    fn symbol(&self) -> String {
        String::from(CONTRACT_SYMBOL)
    }
}

impl Claim {
    pub fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }

    pub fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.name().into_bytes();
        Ok(response)
    }

    pub fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.symbol().into_bytes();
        Ok(response)
    }

    pub fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = MAX_SUPPLY.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_max_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = MAX_SUPPLY.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_minted(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let bb_supply = self.bb_supply_pointer().get_value::<u128>();
        response.data = bb_supply.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_total_staked_by_alkane_id(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        let original_nft_id = self.resolve_alkane_id(&alkane_id)?;
        let total_staked_blocks = self.get_total_staked_blocks_from_contract(&original_nft_id)?;

        response.data = total_staked_blocks.to_le_bytes().to_vec();
        Ok(response)
    }

    pub fn get_total_claimed_by_alkane_id(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        let original_nft_id = self.resolve_alkane_id(&alkane_id)?;
        let claimed_amount = self
            .claimed_amounts_pointer()
            .select(&self.alkane_id_to_bytes(&original_nft_id))
            .get_value::<u128>();

        response.data = claimed_amount.to_le_bytes().to_vec();
        Ok(response)
    }

    pub fn get_total_available_to_claim(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        let original_nft_id = self.resolve_alkane_id(&alkane_id)?;
        let total_rewards = self.calculate_total_rewards(&original_nft_id)?;
        let claimed_amount = self
            .claimed_amounts_pointer()
            .select(&self.alkane_id_to_bytes(&original_nft_id))
            .get_value::<u128>();

        let earned_available = total_rewards.saturating_sub(claimed_amount);
        let remaining_lifetime_limit = SWAP_RATE.saturating_sub(claimed_amount);
        let available = earned_available.min(remaining_lifetime_limit);
        response.data = available.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn claim_rewards(&self) -> Result<CallResponse> {
        let context = self.context()?;

        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("Must provide alkane IDs to claim rewards for"));
        }

        let mut total_claimed = 0u128;

        for alkane in &context.incoming_alkanes.0 {
            let original_nft_id = self.resolve_alkane_id(&alkane.id)?;
            let total_rewards = self.calculate_total_rewards(&original_nft_id)?;

            let original_nft_bytes = self.alkane_id_to_bytes(&original_nft_id);
            let mut claimed_pointer = self.claimed_amounts_pointer().select(&original_nft_bytes);
            let previously_claimed = claimed_pointer.get_value::<u128>();

            if total_rewards <= previously_claimed {
                return Err(anyhow!(
                    "No rewards available: claimed ({}) equals or exceeds total rewards ({})",
                    previously_claimed,
                    total_rewards
                ));
            }

            let earned_available = total_rewards.saturating_sub(previously_claimed);
            let remaining_lifetime_limit = SWAP_RATE.saturating_sub(previously_claimed);
            let available = earned_available.min(remaining_lifetime_limit);

            if available > 0 {
                let new_total_claimed = previously_claimed + available;
                claimed_pointer.set_value(new_total_claimed);
                total_claimed += available;
            }
        }

        let mut total_claimed_pointer = self.total_claimed_pointer();
        let current_total = total_claimed_pointer.get_value::<u128>();
        total_claimed_pointer.set_value(current_total + total_claimed);

        let mut response = CallResponse::default();

        if total_claimed > 0 {
            let current_bb_supply = self.bb_supply_pointer().get_value::<u128>();
            if current_bb_supply + total_claimed > MAX_SUPPLY {
                return Err(anyhow!("Would exceed max BB supply"));
            }

            self.bb_supply_pointer()
                .set_value(current_bb_supply + total_claimed);

            response.alkanes.0.push(AlkaneTransfer {
                id: context.myself.clone(),
                value: total_claimed,
            });
        }

        Ok(response)
    }

    pub fn get_total_claimed(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let total_claimed = self.total_claimed_pointer().get_value::<u128>();
        response.data = total_claimed.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_total_available(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let stake_contract_id = self.get_stake_contract_id();
        let total_staked_cellpack = Cellpack {
            target: stake_contract_id,
            inputs: vec![STAKE_GET_TOTAL_STAKED],
        };

        let total_staked = match self.staticcall(
            &total_staked_cellpack,
            &AlkaneTransferParcel::default(),
            self.fuel(),
        ) {
            Ok(total_staked_response) => match total_staked_response.data.try_into() {
                Ok(data) => u128::from_le_bytes(data),
                Err(_) => 0u128,
            },
            Err(_) => 0u128,
        };

        let total_claimed = self.total_claimed_pointer().get_value::<u128>();
        let total_available = total_staked.saturating_sub(total_claimed);
        response.data = total_available.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn swap_b_b_to_beep_boop(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::default();

        let mut total_incoming_bb = 0u128;
        for alkane in &context.incoming_alkanes.0 {
            if alkane.id != context.myself {
                return Err(anyhow!(
                    "Invalid token - only $BB tokens from this contract are accepted"
                ));
            }
            total_incoming_bb += alkane.value;
        }

        if total_incoming_bb == 0 {
            return Err(anyhow!("Must provide $BB tokens to swap"));
        }

        // Calculate how many complete BEEP BOOPs can be obtained
        let beep_boop_amount = total_incoming_bb / SWAP_RATE;

        // Calculate change (remaining $BB after swap)
        let change_amount = total_incoming_bb % SWAP_RATE;

        // Calculate actual $BB used for the swap
        let bb_used_for_swap = beep_boop_amount * SWAP_RATE;

        if beep_boop_amount == 0 {
            return Err(anyhow!(
                "Insufficient $BB tokens to swap for at least 1 BEEP BOOP (need at least {})",
                SWAP_RATE
            ));
        }

        let contract_beep_boop_balance = self
            .contract_beep_boop_balance_pointer()
            .get_value::<u128>();
        if contract_beep_boop_balance < beep_boop_amount {
            return Err(anyhow!("Insufficient BEEP BOOP tokens in contract pool"));
        }

        // Decrease the $BB supply since tokens are being burned/swapped back
        let current_bb_supply = self.bb_supply_pointer().get_value::<u128>();
        self.bb_supply_pointer()
            .set_value(current_bb_supply - bb_used_for_swap);

        let beep_boop_tokens = self.retrieve_beep_boop_tokens_from_contract(beep_boop_amount)?;

        let mut contract_beep_boop_balance = self
            .contract_beep_boop_balance_pointer()
            .get_value::<u128>();
        contract_beep_boop_balance -= beep_boop_amount;
        self.contract_beep_boop_balance_pointer()
            .set_value(contract_beep_boop_balance);

        response.alkanes.0.extend(beep_boop_tokens);

        // Return change to user if any
        if change_amount > 0 {
            response.alkanes.0.push(AlkaneTransfer {
                id: context.myself.clone(),
                value: change_amount,
            });
        }

        Ok(response)
    }

    pub fn swap_beep_boop_to_b_b(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::default();

        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("No BEEP BOOP tokens provided for swap"));
        }

        let mut total_incoming_beep_boop = 0u128;
        for alkane in &context.incoming_alkanes.0 {
            if !self.verify_id_collection(&alkane.id) {
                return Err(anyhow!(
                    "Invalid token - only BEEP BOOP NFTs can be swapped"
                ));
            }
            total_incoming_beep_boop += alkane.value;
        }

        let bb_amount = total_incoming_beep_boop * SWAP_RATE;

        for alkane in &context.incoming_alkanes.0 {
            self.store_beep_boop_token_in_contract(&alkane.id, alkane.value)?;
        }

        let mut contract_beep_boop_balance = self
            .contract_beep_boop_balance_pointer()
            .get_value::<u128>();
        contract_beep_boop_balance += total_incoming_beep_boop;
        self.contract_beep_boop_balance_pointer()
            .set_value(contract_beep_boop_balance);

        let current_bb_supply = self.bb_supply_pointer().get_value::<u128>();
        if current_bb_supply + bb_amount > MAX_SUPPLY {
            return Err(anyhow!("Would exceed max BB supply"));
        }

        self.bb_supply_pointer()
            .set_value(current_bb_supply + bb_amount);

        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: bb_amount,
        });

        Ok(response)
    }

    pub fn get_beep_boop_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // BEEP BOOP supply is the same as contract balance (count of stored NFTs)
        let supply = self
            .contract_beep_boop_balance_pointer()
            .get_value::<u128>();
        response.data = supply.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_swap_rate(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = SWAP_RATE.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_collection_identifier(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let identifier = format!("{}:{}", context.myself.block, context.myself.tx);
        response.data = identifier.into_bytes();

        Ok(response)
    }

    pub fn deposit_beep_boop(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::default();

        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("Must provide BEEP BOOP tokens to deposit"));
        }

        let mut total_beep_boop_deposited = 0u128;

        for alkane in &context.incoming_alkanes.0 {
            if !self.verify_id_collection(&alkane.id) {
                return Err(anyhow!(
                    "Invalid token - only BEEP BOOP NFTs can be deposited"
                ));
            }
        }

        for alkane in &context.incoming_alkanes.0 {
            self.store_beep_boop_token_in_contract(&alkane.id, alkane.value)?;
            total_beep_boop_deposited += alkane.value;
        }

        let mut contract_beep_boop_balance_pointer = self.contract_beep_boop_balance_pointer();
        let current_contract_beep_boop_balance =
            contract_beep_boop_balance_pointer.get_value::<u128>();
        contract_beep_boop_balance_pointer
            .set_value(current_contract_beep_boop_balance + total_beep_boop_deposited);

        Ok(response)
    }

    pub fn get_next_swap_index(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let next_swap_index = self.next_swap_index_pointer().get_value::<u128>();
        response.data = next_swap_index.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_stored_beep_boop_alkane_id(&self, index: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let key_bytes = index.to_le_bytes().to_vec();
        let token_data = self
            .contract_stored_beep_boop_tokens_pointer()
            .select(&key_bytes)
            .get();

        if token_data.is_empty() {
            return Err(anyhow!("No BEEP BOOP token stored at index {}", index));
        }

        let token_id = self.bytes_to_nft_id(&token_data)?;
        let identifier = format!("{}:{}", token_id.block, token_id.tx);
        response.data = identifier.into_bytes();

        Ok(response)
    }

    fn calculate_total_rewards(&self, alkane_id: &AlkaneId) -> Result<u128> {
        let total_staked_blocks = self
            .get_total_staked_blocks_from_contract(alkane_id)
            .unwrap_or(0);
        let current_staking_blocks = self.get_current_staking_period(alkane_id).unwrap_or(0);
        let total_rewards = total_staked_blocks + current_staking_blocks;

        Ok(total_rewards)
    }

    fn get_total_staked_blocks_from_contract(&self, alkane_id: &AlkaneId) -> Result<u128> {
        let stake_contract_id = self.get_stake_contract_id();
        let cellpack = Cellpack {
            target: stake_contract_id,
            inputs: vec![STAKE_GET_TOTAL_STAKED_BLOCKS, alkane_id.block, alkane_id.tx],
        };

        let response =
            match self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel()) {
                Ok(response) => response,
                Err(_) => return Ok(0u128), // Return 0 if staticcall fails
            };

        if response.data.len() != 16 {
            return Ok(0u128); // Return 0 if response size is invalid
        }

        let total_blocks = match response.data.try_into() {
            Ok(data) => u128::from_le_bytes(data),
            Err(_) => 0u128, // Return 0 if data parsing fails
        };

        Ok(total_blocks)
    }

    fn get_current_staking_period(&self, alkane_id: &AlkaneId) -> Result<u128> {
        let staked_height = self.try_get_staked_height(alkane_id).unwrap_or(0);

        if staked_height == 0 {
            return Ok(0); // Not staked or failed to get height
        }

        let current_height = u128::from(self.height());
        if current_height >= staked_height {
            Ok(current_height - staked_height)
        } else {
            Ok(0)
        }
    }

    fn try_get_staked_height(&self, alkane_id: &AlkaneId) -> Result<u128> {
        let stake_contract_id = self.get_stake_contract_id();
        let cellpack = Cellpack {
            target: stake_contract_id,
            inputs: vec![STAKE_GET_STAKED_HEIGHT, alkane_id.block, alkane_id.tx],
        };

        let response =
            match self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel()) {
                Ok(response) => response,
                Err(_) => return Err(anyhow!("Staticcall failed")), // Return error to be caught by caller
            };

        if response.data.len() != 16 {
            return Err(anyhow!("Invalid response size from GetStakedHeight"));
        }

        let staked_height = match response.data.try_into() {
            Ok(data) => u128::from_le_bytes(data),
            Err(_) => return Err(anyhow!("Failed to parse GetStakedHeight response")),
        };

        if staked_height == 0 {
            return Err(anyhow!("Not currently staked"));
        }

        Ok(staked_height)
    }

    fn get_stake_contract_id(&self) -> AlkaneId {
        AlkaneId {
            block: BEEP_BOOP_STAKE_CONTRACT_BLOCK,
            tx: BEEP_BOOP_STAKE_CONTRACT_TX,
        }
    }

    pub fn alkane_id_to_bytes(&self, alkane_id: &AlkaneId) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&alkane_id.block.to_le_bytes());
        bytes.extend_from_slice(&alkane_id.tx.to_le_bytes());
        bytes
    }

    pub fn verify_id_collection(&self, orbital_id: &AlkaneId) -> bool {
        let stake_contract_id = self.get_stake_contract_id();

        // First try eligibility check - if returns 1, it's definitely valid unstaked BEEP BOOP
        let cellpack = Cellpack {
            target: stake_contract_id,
            inputs: vec![STAKE_GET_ELIGIBILITY, orbital_id.block, orbital_id.tx],
        };

        let response = self
            .staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())
            .unwrap();

        if response.data[0] == 1 {
            return true;
        }

        self.verify_lp_token(orbital_id)
    }

    fn verify_lp_token(&self, lp_id: &AlkaneId) -> bool {
        let stake_contract_id = self.get_stake_contract_id();

        let cellpack = Cellpack {
            target: stake_contract_id,
            inputs: vec![STAKE_GET_STAKED_BY_LP, lp_id.block, lp_id.tx],
        };

        let response = self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel());

        match response {
            Ok(resp) => !resp.data.is_empty(),
            Err(_) => false,
        }
    }

    fn get_original_nft_from_lp(&self, lp_id: &AlkaneId) -> Result<AlkaneId> {
        let stake_contract_id = self.get_stake_contract_id();

        let cellpack = Cellpack {
            target: stake_contract_id,
            inputs: vec![STAKE_GET_STAKED_BY_LP, lp_id.block, lp_id.tx],
        };

        let response = self
            .staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())
            .unwrap();

        let id_string = String::from_utf8(response.data).unwrap();
        let parts: Vec<&str> = id_string.split(':').collect();

        if parts.len() != 2 {
            return Err(anyhow!("Invalid LP token response format"));
        }

        let block = parts[0].parse::<u128>().unwrap();
        let tx = parts[1].parse::<u128>().unwrap();

        Ok(AlkaneId { block, tx })
    }

    fn get_value_per_mint(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = SWAP_RATE.to_le_bytes().to_vec();
        Ok(response)
    }

    fn resolve_alkane_id(&self, id: &AlkaneId) -> Result<AlkaneId> {
        let stake_contract_id = self.get_stake_contract_id();

        let cellpack = Cellpack {
            target: stake_contract_id,
            inputs: vec![STAKE_GET_ELIGIBILITY, id.block, id.tx],
        };

        let response = self
            .staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())
            .unwrap();

        if response.data[0] == 1 {
            return Ok(*id);
        }

        self.get_original_nft_from_lp(id)
    }

    fn store_beep_boop_token_in_contract(&self, token_id: &AlkaneId, _value: u128) -> Result<()> {
        let mut deposit_index_pointer = self.deposit_index_pointer();
        let current_deposit_index = deposit_index_pointer.get_value::<u128>();

        let token_data = self.nft_id_to_bytes(token_id);

        let key_bytes = current_deposit_index.to_le_bytes().to_vec();

        self.contract_stored_beep_boop_tokens_pointer()
            .select(&key_bytes)
            .set(Arc::new(token_data));

        deposit_index_pointer.set_value(current_deposit_index + 1);

        Ok(())
    }

    fn retrieve_beep_boop_tokens_from_contract(&self, amount: u128) -> Result<Vec<AlkaneTransfer>> {
        let mut tokens = Vec::new();
        let mut remaining_nfts = amount;

        let mut swap_index_pointer = self.next_swap_index_pointer();
        let mut current_swap_index = swap_index_pointer.get_value::<u128>();
        let deposit_index = self.deposit_index_pointer().get_value::<u128>();

        while current_swap_index < deposit_index && remaining_nfts > 0 {
            let key_bytes = current_swap_index.to_le_bytes().to_vec();
            let token_data = self
                .contract_stored_beep_boop_tokens_pointer()
                .select(&key_bytes)
                .get();

            if token_data.len() > 0 {
                let token_id = self.bytes_to_nft_id(&token_data)?;

                tokens.push(AlkaneTransfer {
                    id: token_id,
                    value: 1,
                });
                remaining_nfts -= 1;

                self.contract_stored_beep_boop_tokens_pointer()
                    .select(&key_bytes)
                    .set(Arc::new(Vec::new()));

                current_swap_index += 1;
            } else {
                current_swap_index += 1;
            }
        }

        swap_index_pointer.set_value(current_swap_index);

        if remaining_nfts > 0 {
            return Err(anyhow!("Insufficient stored BEEP BOOP NFTs"));
        }

        Ok(tokens)
    }

    fn nft_id_to_bytes(&self, token_id: &AlkaneId) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&token_id.block.to_le_bytes());
        bytes.extend_from_slice(&token_id.tx.to_le_bytes());
        bytes
    }

    fn bytes_to_nft_id(&self, bytes: &[u8]) -> Result<AlkaneId> {
        if bytes.len() != 32 {
            return Err(anyhow!("Invalid NFT data length"));
        }

        let block = u128::from_le_bytes(bytes[0..16].try_into().unwrap());
        let tx = u128::from_le_bytes(bytes[16..32].try_into().unwrap());

        Ok(AlkaneId { block, tx })
    }

    // Storage pointers

    /// Storage pointer for claimed amounts by alkane ID
    fn claimed_amounts_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/claimed-amounts")
    }

    /// Storage pointer for total claimed amount across all alkanes
    fn total_claimed_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total-claimed")
    }

    /// Storage pointer for $BB token supply
    fn bb_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/bb-supply")
    }

    /// Storage pointer for contract's BEEP BOOP token balance
    fn contract_beep_boop_balance_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/contract-beep-boop-balance")
    }

    /// Storage pointer for contract's stored BEEP BOOP tokens
    fn contract_stored_beep_boop_tokens_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/contract-stored-beep-boop-tokens")
    }

    /// Storage pointer for next deposit index (where to store next BEEP BOOP)
    fn deposit_index_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/next-deposit-index")
    }

    /// Storage pointer for next swap index (which BEEP BOOP to give out next)
    fn next_swap_index_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/next-swap-index")
    }
}

declare_alkane! {
    impl AlkaneResponder for Claim {
        type Message = ClaimMessage;
    }
}
