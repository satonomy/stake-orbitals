use image::{self, imageops, DynamicImage, ImageFormat, RgbaImage};
use std::io::Cursor;

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

mod orbitals_ids;
use orbitals_ids::BEEP_BOOP_IDS;

const ORBITAL_TEMPLATE_ID: u128 = 16802;
const MAX_MINTS: u128 = 10000;
const BEEP_BOOP_BLOCK: u128 = 2;
const BEEP_BOOP_COLLECTION_ID: u128 = 31064;
const CONTRACT_NAME: &str = "🖨️ Boop Quantum Vault";
const CONTRACT_SYMBOL: &str = "Beep Boop Orbiting";
const OVERLAY_BYTES: &[u8] = include_bytes!("../assets/overlay.png");

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

    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,

    /// Get the total count of orbitals
    #[opcode(102)]
    #[returns(u128)]
    GetOrbitalCount,

    /// Get the minted count of orbitals
    #[opcode(103)]
    #[returns(u128)]
    GetOrbitalMinted,

    /// Stake an orbital
    #[opcode(500)]
    Stake,

    /// Unstake an orbital
    #[opcode(501)]
    Unstake,

    /// Check if an orbital is eligible to be staked
    #[opcode(506)]
    #[returns(u128)]
    GetStakeEligibility { block: u128, tx: u128 },

    /// Get the stake height (block number when staked)
    #[opcode(507)]
    #[returns(u128)]
    GetStakedHeight { block: u128, tx: u128 },

    /// Get the orbital IDs staked by an lp id
    #[opcode(508)]
    #[returns(String)]
    GetStakedByLp { block: u128, tx: u128 },

    /// Get the total staked blocks for an orbital
    #[opcode(510)]
    #[returns(u128)]
    GetTotalStakedBlocks { block: u128, tx: u128 },

    /// Get the total number of staked orbitals
    #[opcode(511)]
    #[returns(u128)]
    GetTotalStaked,

    #[opcode(512)]
    #[returns(u128)]
    GetTotalUnstaked,

    //
    /// Get the collection identifier
    #[opcode(998)]
    #[returns(String)]
    GetCollectionIdentifier,

    /// Get PNG data for a specific orbital
    ///
    /// # Arguments
    /// * `index` - The index of the orbital
    #[opcode(1000)]
    #[returns(Vec<u8>)]
    GetData { index: u128 },

    /// Get attributes for a specific orbital
    ///
    /// # Arguments
    /// * `index` - The index of the orbital
    #[opcode(1002)]
    #[returns(String)]
    GetAttributes { index: u128 },
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

        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("Must send at least 1 orbital to stake"));
        }

        for alkane in &context.incoming_alkanes.0 {
            if !self.verify_id_collection(&alkane.id) {
                return Err(anyhow!("Alkane ID not verified"));
            }
            let key = self.alkane_id_to_bytes(&alkane.id);
            if self.stake_height_pointer().select(&key).get_value::<u128>() != 0 {
                return Err(anyhow!(
                    "Orbital {}:{} already staked",
                    alkane.id.block,
                    alkane.id.tx
                ));
            }
        }

        let mut minted_lp_orbitals = Vec::new();

        // let mocked_alkanes_transfer = vec![AlkaneTransfer {
        //     id: AlkaneId {
        //         block: 4,
        //         tx: 16802,
        //     },
        //     value: 1,
        // }];

        // for alkane in &mocked_alkanes_transfer {
        for alkane in &context.incoming_alkanes.0 {
            if alkane.value != 1 {
                return Err(anyhow!("Alkane amount must be 1"));
            }

            // call GetNftIndex 999
            let cellpack = Cellpack {
                target: alkane.id,
                inputs: vec![999],
            };

            let call_response =
                self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;

            let index = u128::from_le_bytes(call_response.data.try_into().unwrap());

            // Set stake block pointer for this alkane
            self.stake_height_pointer()
                .select(&self.alkane_id_to_bytes(&alkane.id))
                .set_value(u128::from(self.height())); // this should be u128

            let minted_lp_id = self
                .instances_pointer()
                .select(&index.to_le_bytes().to_vec())
                .get();

            if minted_lp_id.len() == 0 {
                let minted = self.create_mint_transfer(index)?;
                minted_lp_orbitals.push(minted);
                self.set_staked_by_id(&self.alkane_id_to_bytes(&minted.id), alkane.id)?;
            } else {
                let minted_lp_id_bytes = minted_lp_id.as_ref();

                let existing_alkane_id = AlkaneId {
                    block: u128::from_le_bytes(minted_lp_id_bytes[0..16].try_into().unwrap()),
                    tx: u128::from_le_bytes(minted_lp_id_bytes[16..32].try_into().unwrap()),
                };

                let existing_alkane = AlkaneTransfer {
                    id: existing_alkane_id,
                    value: 1u128,
                };
                minted_lp_orbitals.push(existing_alkane.clone());

                self.set_staked_by_id(&minted_lp_id_bytes, alkane.id)?;
            };
        }

        // Increment total staked count
        let mut total_staked_pointer = self.total_staked_pointer();
        let current_total = total_staked_pointer.get_value::<u128>();
        total_staked_pointer
            .set_value(current_total + u128::try_from(context.incoming_alkanes.0.len()).unwrap());

        // clean incoming alkanes set as default
        let mut response = CallResponse::default();

        response.alkanes.0.extend(minted_lp_orbitals);

        Ok(response)
    }

    pub fn unstake(&self) -> Result<CallResponse> {
        let context = self.context()?;

        // Validate incoming alkanes
        if context.incoming_alkanes.0.is_empty() {
            return Err(anyhow!("Must send exactly 1 LP token to unstake"));
        }

        if context.incoming_alkanes.0.len() != 1 {
            return Err(anyhow!("Must send exactly 1 LP token to unstake"));
        }

        if context.incoming_alkanes.0[0].value != 1 {
            return Err(anyhow!("LP token amount must be 1"));
        }
        // let index_key = 0u128.to_le_bytes().to_vec();
        // let first_lp_id_bytes_arc = self.instances_pointer().select(&index_key).get();

        // if first_lp_id_bytes_arc.len() == 0 {
        //     return Err(anyhow!("No LP token found at index 0"));
        // }

        // let bytes = first_lp_id_bytes_arc.as_ref();
        // if 0 return error

        // let first_incoming_alkane_id = AlkaneId { block: 2, tx: 53 };

        // let first_incoming_alkane_id = AlkaneId { block: 2, tx: 41 };

        let first_incoming_alkane_id = context.incoming_alkanes.0[0].id;
        let lp_alkane_id_key = self.alkane_id_to_bytes(&first_incoming_alkane_id);
        let staked_alkane_id = self.get_staked_orbital_id_by_lp_id(&lp_alkane_id_key);

        let staked_at_block = self
            .stake_height_pointer()
            .select(&self.alkane_id_to_bytes(&staked_alkane_id))
            .get_value::<u128>();

        if staked_at_block == 0 {
            return Err(anyhow!("Orbital stake record not found"));
        }

        let period_blocks = u128::from(self.height()).saturating_sub(staked_at_block);

        let mut total_staked_blocks_pointer = self
            .total_staked_blocks_pointer()
            .select(&self.alkane_id_to_bytes(&staked_alkane_id));
        let previous_total = total_staked_blocks_pointer.get_value::<u128>();
        let new_total = previous_total
            .checked_add(period_blocks)
            .ok_or_else(|| anyhow!("Total staked blocks overflow"))?;
        total_staked_blocks_pointer.set_value(new_total);

        // clean the stake height record
        self.stake_height_pointer()
            .select(&self.alkane_id_to_bytes(&staked_alkane_id))
            .set_value(0u128);
        self.staked_id_pointer()
            .select(&lp_alkane_id_key)
            .set(Arc::new(vec![]));

        // Fix: Properly increment the total unstaked counter
        let total_unstaked_current = self.total_unstaked_pointer().get_value::<u128>();
        let new_total_unstaked = total_unstaked_current
            .checked_add(1)
            .ok_or_else(|| anyhow!("Total unstaked overflow"))?;
        self.total_unstaked_pointer().set_value(new_total_unstaked);

        let mut response = CallResponse::default();
        response.alkanes.0.push(AlkaneTransfer {
            id: staked_alkane_id,
            value: 1u128,
        });

        Ok(response)
    }

    fn create_mint_transfer(&self, index: u128) -> Result<AlkaneTransfer> {
        let max_total = self.max_mints();

        if index >= max_total {
            return Err(anyhow!("Minted out"));
        }

        let cellpack = Cellpack {
            target: AlkaneId {
                block: 6,
                tx: ORBITAL_TEMPLATE_ID,
            },
            inputs: vec![0x0, index],
        };

        let sequence = self.sequence();
        let response = self.call(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;

        let orbital_id = AlkaneId {
            block: 2,
            tx: sequence,
        };

        self.add_instance(&orbital_id)?;

        if response.alkanes.0.len() < 1 {
            Err(anyhow!("orbital token not returned with factory"))
        } else {
            Ok(response.alkanes.0[0])
        }
    }

    pub fn get_total_staked_blocks(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        if !self.verify_id_collection(&alkane_id) {
            return Err(anyhow!("Orbital ID not from {}", CONTRACT_NAME));
        }

        let total_staked_blocks = self
            .total_staked_blocks_pointer()
            .select(&self.alkane_id_to_bytes(&alkane_id))
            .get_value::<u128>();

        response.data = total_staked_blocks.to_le_bytes().to_vec();
        Ok(response)
    }

    fn get_staked_orbital_id_by_lp_id(&self, key: &Vec<u8>) -> AlkaneId {
        let pointer = self.staked_id_pointer().select(key);
        let data = pointer.get();

        if data.len() == 0 {
            panic!("Beep Boop LP ID not found");
        }

        let bytes = data.as_ref();

        let alkane_id = AlkaneId {
            block: u128::from_le_bytes(bytes[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(bytes[16..32].try_into().unwrap()),
        };

        alkane_id
    }

    fn get_staked_by_lp(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let pointer = self
            .staked_id_pointer()
            .select(&self.alkane_id_to_bytes(&AlkaneId { block, tx }));
        let arc_bytes = pointer.get();

        if arc_bytes.len() == 0 {
            return Err(anyhow!("LP token has no staked orbital"));
        }

        let alkane_id_string = format!(
            "{}:{}",
            u128::from_le_bytes(arc_bytes[0..16].try_into().unwrap()),
            u128::from_le_bytes(arc_bytes[16..32].try_into().unwrap())
        );

        response.data = alkane_id_string.into_bytes();

        Ok(response)
    }

    fn set_staked_by_id(&self, minted_lp_id_bytes: &Vec<u8>, alkane_id: AlkaneId) -> Result<()> {
        let mut staked_pointer = self.staked_id_pointer().select(&minted_lp_id_bytes);
        let alkane_id_bytes = self.alkane_id_to_bytes(&alkane_id);
        staked_pointer.set(Arc::new(alkane_id_bytes));
        Ok(())
    }

    /// Check if an orbital is eligible to be staked.
    pub fn get_stake_eligibility(&self, block: u128, tx: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let alkane_id = AlkaneId { block, tx };
        // let eligible = self
        //     .stake_height_pointer()
        //     .select(&self.alkane_id_to_bytes(&alkane_id))
        //     .get_value::<u128>()
        //     == 0;
        let eligible = self.verify_id_collection(&alkane_id)
            && self
                .stake_height_pointer()
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
            .stake_height_pointer()
            .select(&alkane_id_bytes)
            .get_value::<u128>();
        if staked_at_block == 0 {
            return Err(anyhow!("Orbital not staked"));
        }

        response.data = staked_at_block.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_total_staked(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let total_staked = self.total_staked_pointer().get_value::<u128>();
        response.data = total_staked.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_total_unstaked(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let total_unstaked = self.total_unstaked_pointer().get_value::<u128>();
        response.data = total_unstaked.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // Total supply is the current instances count
        response.data = self.instances_count().to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_orbital_count(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = MAX_MINTS.to_le_bytes().to_vec();

        Ok(response)
    }

    pub fn get_orbital_minted(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // Calculate actual minted count = total instances count - authorized mint count
        let minted_count = self.instances_count();
        response.data = minted_count.to_le_bytes().to_vec();

        Ok(response)
    }

    fn max_mints(&self) -> u128 {
        MAX_MINTS
    }

    fn get_data(&self, index: u128) -> Result<CallResponse> {
        let ctx = self.context()?;
        let mut response = CallResponse::forward(&ctx.incoming_alkanes);

        let collection_id = AlkaneId {
            block: BEEP_BOOP_BLOCK,
            tx: BEEP_BOOP_COLLECTION_ID,
        };

        let cell = Cellpack {
            target: collection_id,
            inputs: vec![1000, index],
        };
        let base_png = self
            .staticcall(&cell, &AlkaneTransferParcel::default(), self.fuel())?
            .data;

        // decode both images
        let mut base: RgbaImage = image::load_from_memory(&base_png)?.to_rgba8();
        let overlay: RgbaImage = image::load_from_memory(OVERLAY_BYTES)?.to_rgba8();

        // Get overlay dimensions
        let (overlay_width, overlay_height) = overlay.dimensions();

        // Center the overlay on the base image
        let x_offset = (420 - overlay_width) / 2;
        let y_offset = (420 - overlay_height) / 2;

        imageops::overlay(&mut base, &overlay, x_offset as i64, y_offset as i64);

        // re-encode
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(base).write_to(&mut Cursor::new(&mut out), ImageFormat::Png)?;

        response.data = out;
        Ok(response)
    }

    fn get_attributes(&self, index: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let collection_id = AlkaneId {
            block: BEEP_BOOP_BLOCK,
            tx: BEEP_BOOP_COLLECTION_ID,
        };

        let cellpack = Cellpack {
            target: collection_id,
            inputs: vec![1002, index],
        };

        let call_response =
            self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;

        response.data = call_response.data;

        Ok(response)
    }

    /// Get the collection identifier
    /// Returns the collection identifier in the format "block:tx"
    fn get_collection_identifier(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // Format the collection identifier as "block:tx"
        let identifier = format!("{}:{}", context.myself.block, context.myself.tx);
        response.data = identifier.into_bytes();

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

    fn add_instance(&self, instance_id: &AlkaneId) -> Result<u128> {
        let count = self.instances_count();
        let new_count = count.checked_add(1).ok_or_else(|| anyhow!("Minted out"))?;

        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&instance_id.block.to_le_bytes());
        bytes.extend_from_slice(&instance_id.tx.to_le_bytes());

        let bytes_vec = new_count.to_le_bytes().to_vec();
        let mut instance_pointer = self.instances_pointer().select(&bytes_vec);
        instance_pointer.set(Arc::new(bytes));

        self.set_instances_count(new_count);

        Ok(new_count)
    }

    fn instances_count(&self) -> u128 {
        self.instances_pointer().get_value::<u128>()
    }

    fn set_instances_count(&self, count: u128) {
        self.instances_pointer().set_value::<u128>(count);
    }

    fn instances_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/instances")
    }

    /// Get storage pointer for stake height records
    pub fn stake_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/stake-height")
    }

    /// Get storage pointer for address-to-staked-orbital mapping
    pub fn staked_id_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/address-staked-pointer")
    }

    pub fn total_staked_blocks_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total-staked-blocks")
    }

    pub fn total_staked_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total-staked")
    }

    pub fn total_unstaked_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total-unstaked")
    }
}

declare_alkane! {
    impl AlkaneResponder for Staking {
        type Message = StakingMessage;
    }
}
