# Satonomy Beep Boop Stake 🤖

Stake, unstake, and query staked block counts on the Alkanes protocol.

📈 Live demo: [satonomy.io/staking](https://satonomy.io/staking)

## Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Deploy

```bash
oyl alkane new-contract \
  -c ./target/alkanes/wasm32-unknown-unknown/release/alkanes_stake.wasm \
  -data 1,0 \
  -p oylnet
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
    "txid":"c772fc3f70c489401951c07cc8e640f95e4532a4164f6f221035d4997b182dc7",
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
  -params '{"block":"2","tx":"10"}' \
  -p oylnet
```

```json
{
  "name": "Stake Beep Boop",
  "symbol": "📠",
  "mintActive": false,
  "percentageMinted": null,
  "totalSupply": 0,
  "cap": 0,
  "minted": 0,
  "mintAmount": 0
}
```

## Simulate Transactions

Verify an Alkane ID:

```bash
oyl provider alkanes \
-method simulate \
-params '{
  "target": { "block": "2", "tx": "10" },
  "inputs": ["506", "2", "615"]
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

## Community

- Discord: [discord.gg/7Qcs4qhSZr](https://discord.gg/7Qcs4qhSZr)
- X: [@estevanbtc](https://x.com/estevanbtc)
