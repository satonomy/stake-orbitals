use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
    token::Token,
};
use alkanes_support::{id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse};
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;

use anyhow::{anyhow, Result};
use bitcoin::Transaction;
use metashrew_support::utils::consensus_decode;
use std::io::Cursor;
use std::sync::Arc;

mod orbitals_ids;
use orbitals_ids::BEEP_BOOP_IDS;

const BEEP_BOOP_BLOCK: u128 = 0x2;
const CONTRACT_NAME: &str = "Stake Beep Boop";
const CONTRACT_SYMBOL: &str = "📠";

#[derive(Default)]
pub struct Staking(());

/// Implementation of AlkaneResponder trait for the collection
impl AlkaneResponder for Staking {}

/// Message types for contract interaction
/// These messages define the available operations that can be performed on the contract
#[derive(MessageDispatch)]
enum StakingMessage {
    /// Initialize the contract and perform premine
    #[opcode(0)]
    Initialize,

    /// Get the name of the collection
    #[opcode(99)]
    #[returns(String)]
    GetName,

    /// Get the symbol of the collection
    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    /// Stake an orbital
    #[opcode(500)]
    Stake,

    /// Unstake an orbital
    #[opcode(501)]
    Unstake { block: u128, tx: u128 },

    /// Check if an orbital is eligible to be staked
    #[opcode(506)]
    #[returns(u128)]
    GetStakeEligibility { block: u128, tx: u128 },

    /// Get the stake height (block number when staked)
    #[opcode(507)]
    #[returns(u128)]
    GetStakedHeight { block: u128, tx: u128 },

    /// Get the orbital IDs staked by an address
    #[opcode(508)]
    #[returns(String)]
    GetStakedByAddress { lo: u128, hi: u128 },

    /// Get the unstake height (block number when unstaked)
    #[opcode(509)]
    #[returns(u128)]
    GetUnstakeHeight { block: u128, tx: u128 },

    /// Get the total staked blocks for an orbital
    #[opcode(510)]
    #[returns(u128)]
    GetTotalStakedBlocks { block: u128, tx: u128 },
}

/// Implementation of Token trait
impl Token for Staking {
    /// Returns the name of the token
    fn name(&self) -> String {
        String::from(CONTRACT_NAME)
    }

    /// Returns the symbol of the token
    fn symbol(&self) -> String {
        String::from(CONTRACT_SYMBOL)
    }
}

impl Staking {
    pub fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;

        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 1u128,
        });

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

    /// Public stake function for orbitals
    pub fn stake(&self) -> Result<CallResponse> {
        let context = self.context()?;

        let transaction: Transaction = consensus_decode(&mut Cursor::new(self.transaction()))
            .map_err(|e| anyhow!("tx parse failed: {}", e))?;

        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("Must send at least 1 orbital to stake"));
        }

        let claim_output = transaction
            .output
            .get(0)
            .ok_or_else(|| anyhow!("no outputs"))?;
        let claim_addr = &claim_output.script_pubkey.as_bytes().to_vec();
        let mut staked_alkane_ids = self.get_staked_orbital_ids_by_address(&claim_addr);

        for alkane in &context.incoming_alkanes.0 {
            if !self.verify_id_collection(&alkane.id) {
                return Err(anyhow!("Alkane ID not verified"));
            }

            let pointer_key = self.alkane_id_to_bytes(&alkane.id);
            let already_staked = self
                .stake_block_pointer()
                .select(&pointer_key)
                .get_value::<u128>();

            if already_staked != 0 {
                return Err(anyhow!(
                    "Orbital {}:{} already staked at block {}",
                    alkane.id.block,
                    alkane.id.tx,
                    already_staked
                ));
            }

            // Check if already staked
            if staked_alkane_ids
                .iter()
                .any(|alkane_id| alkane_id.block == alkane.id.block && alkane_id.tx == alkane.id.tx)
            {
                return Err(anyhow!("Orbital already in your stake list"));
            }

            // Add new orbital to existing list
            staked_alkane_ids.push(alkane.id.clone());

            // Set stake block pointer for this alkane
            self.stake_block_pointer()
                .select(&self.alkane_id_to_bytes(&alkane.id))
                .set_value(self.height());
        }

        self.set_staked_by_address(&claim_addr, staked_alkane_ids)?;

        Ok(CallResponse::default())
    }

    pub fn unstake(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let alkane_id = AlkaneId { block, tx };

        if !self.verify_id_collection(&alkane_id) {
            return Err(anyhow!("Orbital ID not from {}", CONTRACT_NAME));
        }

        // Get the address from the transaction
        let transaction: Transaction = consensus_decode(&mut Cursor::new(self.transaction()))
            .map_err(|e| anyhow!("tx parse failed: {}", e))?;
        let claim_output = transaction
            .output
            .get(0)
            .ok_or_else(|| anyhow!("no outputs"))?;
        let claim_addr = &claim_output.script_pubkey.as_bytes().to_vec();

        // Get current staked orbitals for this address
        let mut staked_alkane_ids = self.get_staked_orbital_ids_by_address(claim_addr);

        // Check if the orbital is actually staked
        if !staked_alkane_ids
            .iter()
            .any(|staked_id| staked_id.block == alkane_id.block && staked_id.tx == alkane_id.tx)
        {
            return Err(anyhow!("Orbital is not staked"));
        }

        // Check if stake record exists
        let staked_at_block = self
            .stake_block_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id))
            .get_value::<u128>();

        if staked_at_block == 0 {
            return Err(anyhow!("Orbital stake record not found"));
        }

        let now = self.height();

        // Remove the specific orbital from the list
        staked_alkane_ids.retain(|staked_id| {
            !(staked_id.block == alkane_id.block && staked_id.tx == alkane_id.tx)
        });

        // Update the address storage with the filtered list
        self.set_staked_by_address(claim_addr, staked_alkane_ids)?;

        // Save the unstake height
        self.unstake_height_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id))
            .set_value(self.height());

        // Clean the stake height record
        let mut stake_pointer = self
            .stake_block_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id));
        stake_pointer.nullify();

        let period_blocks = u128::from(now) - staked_at_block;

        let mut total_pointer = self
            .total_staked_blocks_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id));
        let previous_total = total_pointer.get_value::<u128>();
        let new_total = previous_total
            .checked_add(period_blocks)
            .ok_or_else(|| anyhow!("Total staked blocks overflow"))?;
        total_pointer.set_value(new_total);

        let mut response = CallResponse::default();

        response.alkanes.0.push(AlkaneTransfer {
            id: alkane_id,
            value: 1u128,
        });

        Ok(response)
    }

    pub fn get_total_staked_blocks(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        if !self.verify_id_collection(&alkane_id) {
            return Err(anyhow!("Orbital ID not from {}", CONTRACT_NAME));
        }

        let total_blocks = self
            .total_staked_blocks_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id))
            .get_value::<u128>();

        response.data = total_blocks.to_le_bytes().to_vec();
        Ok(response)
    }

    pub fn get_staked_by_address(&self, lo: u128, hi: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let mut address_witness = Vec::with_capacity(32);
        address_witness.extend_from_slice(&hi.to_le_bytes());
        address_witness.extend_from_slice(&lo.to_le_bytes());
        while address_witness.last() == Some(&0) {
            address_witness.pop(); // trim padding zeros
        }

        let orbital_ids = self.get_staked_orbital_ids_by_address(&address_witness);
        let mut staked_ids = Vec::new();
        for id in orbital_ids {
            staked_ids.push(format!("{}:{}", id.block, id.tx));
        }

        // if no ids, return empty
        if staked_ids.is_empty() {
            return Ok(response);
        }

        response.data = staked_ids.join(",").into_bytes();

        Ok(response)
    }

    fn get_staked_orbital_ids_by_address(&self, address_witness: &Vec<u8>) -> Vec<AlkaneId> {
        let pointer = self.address_staked_pointer().select(&address_witness);
        let arc_bytes = pointer.get();

        if arc_bytes.is_empty() {
            return Vec::new();
        }

        arc_bytes
            .chunks_exact(32)
            .map(|chunk| AlkaneId {
                block: u128::from_le_bytes(chunk[0..16].try_into().unwrap()),
                tx: u128::from_le_bytes(chunk[16..32].try_into().unwrap()),
            })
            .collect()
    }

    fn set_staked_by_address(&self, address: &Vec<u8>, alkane_ids: Vec<AlkaneId>) -> Result<()> {
        let mut staked_pointer = self.address_staked_pointer().select(&address);

        let mut multiples_alkane_bytes = Vec::with_capacity(32 * alkane_ids.len());

        for alkane_id in alkane_ids {
            multiples_alkane_bytes.extend_from_slice(&alkane_id.block.to_le_bytes());
            multiples_alkane_bytes.extend_from_slice(&alkane_id.tx.to_le_bytes());
        }

        staked_pointer.set(Arc::new(multiples_alkane_bytes));

        Ok(())
    }

    /// Check if an orbital is eligible to be staked.
    /// Example: alkane_str = "2:4121" returns 1 if eligible, error if not
    pub fn get_stake_eligibility(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        let eligible = self.verify_id_collection(&alkane_id)
            && self
                .stake_block_pointer()
                .select(&self.alkane_id_to_bytes(&alkane_id))
                .get_value::<u128>()
                == 0;

        response.data = vec![eligible as u8];
        Ok(response)
    }

    pub fn get_staked_height(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };

        if !self.verify_id_collection(&alkane_id) {
            return Err(anyhow!("Orbital ID not from {}", CONTRACT_NAME));
        }
        let alkane_id_bytes = self.alkane_id_to_bytes(&alkane_id);
        let staked_at_block = self
            .stake_block_pointer()
            .select(&alkane_id_bytes)
            .get_value::<u128>();
        if staked_at_block == 0 {
            return Err(anyhow!("Orbital not staked"));
        }

        response.data = staked_at_block.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_unstake_height(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };

        if !self.verify_id_collection(&alkane_id) {
            return Err(anyhow!("Orbital ID not from {}", CONTRACT_NAME));
        }

        let alkane_id_bytes = self.alkane_id_to_bytes(&alkane_id);
        let unstake_height = self
            .unstake_height_pointer()
            .select(&alkane_id_bytes)
            .get_value::<u128>();

        if unstake_height == 0 {
            return Err(anyhow!("Orbital has not been unstaked"));
        }

        response.data = unstake_height.to_le_bytes().to_vec();

        Ok(response)
    }

    fn alkane_id_to_bytes(&self, alkane_id: &AlkaneId) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&alkane_id.block.to_le_bytes());
        bytes.extend_from_slice(&alkane_id.tx.to_le_bytes());
        bytes
    }

    fn verify_id_collection(&self, orbital_id: &AlkaneId) -> bool {
        orbital_id.block == BEEP_BOOP_BLOCK && BEEP_BOOP_IDS.contains(&orbital_id.tx)
    }

    /// Get storage pointer for stake height records
    fn stake_block_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/stake-blocks")
    }

    /// Get storage pointer for address-to-staked-orbital mapping
    fn address_staked_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/address-staked-pointer")
    }

    /// Get storage pointer for unstake height records
    fn unstake_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/unstake-heights")
    }

    fn total_staked_blocks_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total-staked-blocks")
    }
}

declare_alkane! {
    impl AlkaneResponder for Staking {
        type Message = StakingMessage;
    }
}
