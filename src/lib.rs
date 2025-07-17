use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
    token::Token,
};
use alkanes_support::{id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse};
use bitcoin;
use bitcoin::blockdata::script::Instruction;

use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;

use anyhow::{anyhow, Result};
use bitcoin::{Transaction, TxOut};
use metashrew_support::utils::consensus_decode;
use std::{io::Cursor, sync::Arc};

mod orbitals_ids;
use orbitals_ids::BEEP_BOOP_IDS;

const BEEP_BOOP_BLOCK: u128 = 0x2;
const CONTRACT_NAME: &str = "Stake Beep Boop";
const CONTRACT_SYMBOL: &str = "📠";
const MIN_STAKE_HEIGHT_DIFF: u128 = 1;

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
    Stake { lo: u128, hi: u128 },

    #[opcode(501)]
    GetStakedIdsByAddress { lo: u128, hi: u128 },

    /// Unstake an orbital
    #[opcode(502)]
    GetStakeRewardsById { block: u128, tx: u128 },

    #[opcode(503)]
    GetStakedOutput {
        lo: u128,
        hi: u128,
        block: u128,
        tx: u128,
    },

    /// Get the total number of staked orbitals
    #[opcode(504)]
    #[returns(u128)]
    GetTotalStaked,

    #[opcode(505)]
    #[returns(u128)]
    GetTotalRewards,
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
    pub fn stake(&self, lo: u128, hi: u128) -> Result<CallResponse> {
        let context = self.context()?;

        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("Must send at least 1 orbital to stake"));
        }

        if context.incoming_alkanes.0.len() > 1 {
            return Err(anyhow!("Must send at most 1 orbital to stake"));
        }

        let alkane_id = &context.incoming_alkanes.0[0].id.clone();

        if !self.verify_id_collection(&alkane_id) {
            return Err(anyhow!("Alkane ID not verified"));
        }

        let transaction: Transaction = consensus_decode(&mut Cursor::new(self.transaction()))
            .map_err(|e| anyhow!("tx parse failed: {}", e))?;

        if transaction.output[0].script_pubkey.is_op_return() {
            return Err(anyhow!("Output[0] cannot be OP_RETURN"));
        }

        let (token_output_index, locked_output) = transaction
            .output
            .iter()
            .enumerate()
            .find_map(|(idx, out)| {
                if out.script_pubkey.is_op_return() {
                    return None;
                }

                let mut found_cltv = false;

                for instr in out.script_pubkey.instructions() {
                    match instr {
                        Ok(Instruction::Op(bitcoin::blockdata::opcodes::all::OP_CLTV)) => {
                            found_cltv = true;
                        }
                        _ => {}
                    }
                }

                if found_cltv {
                    Some((idx, out))
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow!("Could not find Alkane token output"))?;

        // id must be token_output_index
        if token_output_index != 0 {
            return Err(anyhow!("Token output index must be 0"));
        }

        fn extract_locktime_from_stake_output(locked_output: &TxOut) -> Result<u32> {
            let instrs: Vec<_> = locked_output
                .script_pubkey
                .instructions()
                .take(6) // we only expect 5 pushes/opcodes + no more
                .collect::<Result<_, _>>()?; // unwrap any script decode errors

            // now match exactly 5 instructions
            match instrs.as_slice() {
                [Instruction::PushBytes(locktime_bytes), Instruction::Op(bitcoin::blockdata::opcodes::all::OP_CLTV), Instruction::Op(bitcoin::blockdata::opcodes::all::OP_DROP), Instruction::PushBytes(_pubkey_bytes), Instruction::Op(bitcoin::blockdata::opcodes::all::OP_CHECKSIG)] =>
                {
                    // safe to parse!
                    if locktime_bytes.len() == 4 {
                        let arr: [u8; 4] = locktime_bytes
                            .as_bytes()
                            .try_into()
                            .map_err(|_| anyhow!("Locktime must be exactly 4 bytes"))?;
                        Ok(u32::from_le_bytes(arr))
                    } else {
                        Err(anyhow!("Locktime must be exactly 4 bytes"))
                    }
                }
                _ => Err(anyhow!("Not a valid stake‐script for this contract")),
            }
        }

        let locktime = extract_locktime_from_stake_output(&locked_output)?;

        let current_height = self.height();

        if locktime < u32::try_from(current_height).unwrap() {
            return Err(anyhow!("Locktime is in the past"));
        }

        let lock_diff = locktime - u32::try_from(current_height).unwrap();

        if lock_diff < u32::try_from(MIN_STAKE_HEIGHT_DIFF).unwrap() {
            return Err(anyhow!("Not minimum height diff"));
        }

        let mut witness = Vec::with_capacity(32);
        witness.extend_from_slice(&hi.to_le_bytes());
        witness.extend_from_slice(&lo.to_le_bytes());
        witness.resize(32, 0);

        let output_script = locked_output.script_pubkey.as_bytes().to_vec();

        let mut key = witness.clone();
        key.extend_from_slice(&self.alkane_id_to_bytes(&alkane_id));

        self.staked_output_script_pointer()
            .select(&key)
            .set(Arc::new(output_script));

        let address_alkane_ids = self.address_alkane_ids_pointer().select(&witness).get();
        let mut new_address_alkane_ids = address_alkane_ids.as_ref().clone();
        new_address_alkane_ids.extend_from_slice(&self.alkane_id_to_bytes(&alkane_id));

        self.address_alkane_ids_pointer()
            .select(&witness)
            .set(Arc::new(new_address_alkane_ids));

        let incoming_reward_amount = u128::try_from(lock_diff).unwrap();
        let old_reward_amount = self
            .stake_rewards_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id))
            .get_value::<u128>();

        let new_reward_amount = old_reward_amount + incoming_reward_amount;

        self.stake_rewards_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id))
            .set_value(new_reward_amount);

        let mut total_rewards_pointer = self.total_rewards_pointer();
        let current_total = total_rewards_pointer.get_value::<u128>();
        total_rewards_pointer.set_value(current_total + incoming_reward_amount);

        let mut total_staked_pointer = self.total_staked_pointer();
        let current_total = total_staked_pointer.get_value::<u128>();
        total_staked_pointer.set_value(current_total + 1);

        let response = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }

    pub fn get_staked_output(
        &self,
        lo: u128,
        hi: u128,
        block: u128,
        tx: u128,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let alkane_id = AlkaneId { block, tx };

        let mut witness = Vec::with_capacity(32);
        witness.extend_from_slice(&hi.to_le_bytes());
        witness.extend_from_slice(&lo.to_le_bytes());
        witness.resize(32, 0);

        let mut key = witness.clone();
        key.extend_from_slice(&self.alkane_id_to_bytes(&alkane_id));

        let pointer = self.staked_output_script_pointer().select(&key);
        let staked_output_script = pointer.get();

        if staked_output_script.is_empty() {
            return Err(anyhow!("No scripts found for this address"));
        }

        response.data = staked_output_script.as_ref().clone();

        Ok(response)
    }

    pub fn get_staked_ids_by_address(&self, lo: u128, hi: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let mut witness = Vec::with_capacity(32);
        witness.extend_from_slice(&hi.to_le_bytes());
        witness.extend_from_slice(&lo.to_le_bytes());
        witness.resize(32, 0);

        let pointer = self.address_alkane_ids_pointer().select(&witness);
        let address_alkane_ids = pointer.get();

        if address_alkane_ids.is_empty() {
            return Err(anyhow!("No alkane IDs found for this address"));
        }

        response.data = address_alkane_ids.as_ref().clone();

        Ok(response)
    }

    /// Check if an orbital is eligible to be staked.
    pub fn get_stake_eligibility(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        let eligible = self.verify_id_collection(&alkane_id);

        response.data = vec![eligible as u8];
        Ok(response)
    }

    pub fn get_total_staked(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let total_staked = self.total_staked_pointer().get_value::<u128>();
        response.data = total_staked.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_total_rewards(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let total_staked = self.total_rewards_pointer().get_value::<u128>();
        response.data = total_staked.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_stake_rewards_by_id(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        let total_staked = self
            .stake_rewards_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id))
            .get_value::<u128>();

        response.data = total_staked.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn alkane_id_to_bytes(&self, alkane_id: &AlkaneId) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&alkane_id.block.to_le_bytes());
        bytes.extend_from_slice(&alkane_id.tx.to_le_bytes());
        bytes
    }

    pub fn verify_id_collection(&self, orbital_id: &AlkaneId) -> bool {
        orbital_id.block == BEEP_BOOP_BLOCK && BEEP_BOOP_IDS.contains(&orbital_id.tx)
    }

    /// Get storage pointer for staked IDs
    pub fn address_alkane_ids_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/staked-ids")
    }

    /// Get storage pointer for staked rewards records
    pub fn stake_rewards_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/rewards")
    }

    /// Get storage pointer for address-to-staked-orbital mapping
    pub fn address_staked_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/address-pointer")
    }

    pub fn staked_output_script_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/staked-output")
    }

    pub fn total_staked_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total-staked")
    }

    pub fn total_rewards_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total-rewards")
    }
}

declare_alkane! {
    impl AlkaneResponder for Staking {
        type Message = StakingMessage;
    }
}
