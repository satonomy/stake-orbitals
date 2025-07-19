# Boop Quantum Vault 🤖

A sophisticated NFT staking contract for Beep Boop orbitals on the Alkanes protocol. Stake your orbitals, earn LP tokens, and track staking rewards over time.

📈 Live demo: [satonomy.io/staking](https://satonomy.io/staking)

## Overview

The **Boop Quantum Vault** (symbol: "Ultra Beep Boop") is an advanced staking contract that allows users to stake Beep Boop orbital NFTs and receive LP (Liquidity Provider) tokens in return. The contract tracks staking duration, manages image composition, and provides comprehensive querying capabilities.

### Contract Metadata

- **Name**: Boop Quantum Vault
- **Symbol**: Ultra Beep Boop
- **Max Supply**: 10,000 tokens
- **Base Collection ID**: 2:31064 (Satonomy Beep Boop)

## Key Features

### 🔒 **Staking Mechanics**

- **Stake Orbitals** (opcode 500): Deposit Beep Boop orbitals to receive LP tokens
- **Unstake Orbitals** (opcode 501): Return LP tokens to reclaim original orbitals
- **Eligibility Checking**: Verify if an orbital can be staked
- **Double-Stake Prevention**: Ensures orbitals can't be staked multiple times

### 📊 **Tracking & Analytics**

- **Staking Height**: Records the block number when each orbital was staked
- **Total Staked Blocks**: Accumulates staking time for reward calculations
- **LP to Orbital Mapping**: Tracks which orbital each LP token represents
- **Collection Statistics**: Total supply, minted count, and staked metrics

### 🖼️ **Dynamic Image Generation**

- Fetches base PNG from the Beep Boop collection
- Overlays custom staking imagery (`assets/overlay.png`)
- Centers overlay on 420x420 base images
- Returns composited PNG data for staked tokens

### 🔐 **Security Features**

- **ID Verification**: Only verified Beep Boop orbitals can be staked
- **Amount Validation**: Enforces exactly 1 NFT per stake operation
- **Overflow Protection**: Uses checked arithmetic throughout
- **State Consistency**: Maintains accurate mapping between LP tokens and orbitals

## Contract Architecture

### Storage Pointers

```rust
/instances              // Minted LP token count and mappings
/stake-height          // When each orbital was staked
/address-staked-pointer // LP token to orbital mapping
/total-staked-blocks   // Cumulative staking time per orbital
/total-staked          // Total number of staked orbitals
/total-unstaked        // Total number of unstaked orbitals
```

### Available Operations

| Opcode | Function                | Description                              |
| ------ | ----------------------- | ---------------------------------------- |
| 0      | Initialize              | Initialize contract and perform premine  |
| 99     | GetName                 | Returns contract name                    |
| 100    | GetSymbol               | Returns contract symbol                  |
| 101    | GetTotalSupply          | Returns total LP tokens minted           |
| 102    | GetOrbitalCount         | Returns maximum orbital count (10,000)   |
| 103    | GetOrbitalMinted        | Returns current minted count             |
| 500    | Stake                   | Stake orbitals and receive LP tokens     |
| 501    | Unstake                 | Return LP tokens and reclaim orbitals    |
| 506    | GetStakeEligibility     | Check if orbital can be staked           |
| 507    | GetStakedHeight         | Get block when orbital was staked        |
| 508    | GetStakedByLp           | Get orbital ID for given LP token        |
| 510    | GetTotalStakedBlocks    | Get cumulative staked blocks for orbital |
| 511    | GetTotalStaked          | Get total number of staked orbitals      |
| 998    | GetCollectionIdentifier | Returns "block:tx" identifier            |
| 1000   | GetData                 | Returns PNG data with overlay            |
| 1002   | GetAttributes           | Returns orbital attributes               |

## Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Deploy

```bash
oyl alkane new-contract \
  -c ./target/alkanes/wasm32-unknown-unknown/release/alkanes_stake.wasm \
  -data 1,0 \
  -p bitcoin -feeRate 1
```

**Sample response:**

```json
{
  "txId": "c772fc3f70c489401951c07cc8e640f95e4532a4164f6f221035d4997b182dc7",
  "rawTx": "02000000000...",
  "size": 35645,
  "weight": 142580,
  "fee": 70884,
  "satsPerVByte": "1.99",
  "commitTx": "cb35796bc41ae9eefb3eb7b19309fd29438d7231dab413770dd018dbcb178acb"
}
```

## Tracing

```bash
oyl provider alkanes \
  --method trace \
  -params '{
    "txid":"67ad5d9c86b9d0b6c924074611c45d4c6db60c5a631e7b14908df4089c223078",
    "vout":3
  }' \
  -p oylnet
```

**Example output:**

```json
[
  { "event": "create", "data": { "block": "0x2", "tx": "0xa" } },
  {
    "event": "invoke",
    "data": {
      "type": "call",
      "context": {
        "myself": { "block": "0x2", "tx": "0xa" },
        "caller": { "block": "0x0", "tx": "0x0" },
        "inputs": ["0x0", "..."],
        "incomingAlkanes": [],
        "vout": 3
      },
      "fuel": 3500000
    }
  },
  {
    "event": "return",
    "data": {
      "status": "success",
      "response": {
        "alkanes": [{ "id": { "block": "0x2", "tx": "0xa" }, "value": "0x1" }],
        "data": "0x",
        "storage": [{ "key": "/initialized", "value": "0x01" }]
      }
    }
  }
]
```

## Contract ID

From the `create` event:

```json
"myself": { "block": "0x2", "tx": "0xa" }
```

→ **Contract ID**: `2:10`

## Contract Calls

Fetch contract metadata:

```bash
oyl provider alkanes \
  --method getAlkaneById \
  -params '{"block":"4","tx":"16802"}' \
  -p oylnet
```

```json
{
  "name": "Boop Quantum Vault",
  "symbol": "Ultra Beep Boop",
  "mintActive": false,
  "percentageMinted": null,
  "totalSupply": 0,
  "cap": 10000,
  "minted": 0,
  "mintAmount": 0
}
```

## Simulate Transactions

Check if a Beep Boop orbital is eligible for staking:

```bash
oyl provider alkanes \
-method simulate \
-params '{
  "target": { "block": "2", "tx": "YOUR_CONTRACT_ID" },
  "inputs": ["506", "2", "31065"]
}' \
-p oylnet
```

```json
{
  "status": 0,
  "gasUsed": 52508,
  "execution": { "alkanes": [], "storage": [], "error": null, "data": "0x01" },
  "parsed": { "string": "\u0001", "bytes": "0x01", "le": "1", "be": "1" }
}
```

> `status: 0` and `data: 0x01` ⇒ verification succeeded

Execute contract

```bash
oyl alkane execute --calldata 2,32,500 -p oylnet
```

## Community

- Discord: [discord.gg/7Qcs4qhSZr](https://discord.gg/7Qcs4qhSZr)
- X: [@estevanbtc](https://x.com/estevanbtc)
