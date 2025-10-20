# BB

```bash
cargo build --target wasm32-unknown-unknown --release
```

```bash
oyl alkane new-contract \
  -c ./target/alkanes/wasm32-unknown-unknown/release/alkanes_bb.wasm \
  -data 1,0 \
  -p bitcoin
```

```bash
oyl provider alkanes \
  --method trace \
  -params '{
    "txid":"25854c2dc35bf3ca081a791b964af89c4285698357528dc3041308d693da4c13",
    "vout":3
  }' \
  -p bitcoin
```
