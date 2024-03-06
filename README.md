# ord-rs

Ordinal-aware Bitcoin inscriptions wallet and indexer.

## Modules

- [inscription](src/inscription.rs): `Brc20` and `Nft` inscription types.
- [wallet](src/wallet.rs): Transaction builder, signer, and parser.
- [indexer](src/indexer.rs): Scans the Bitcoin blockchain to catalog, organize, and provide information about Ordinal inscriptions.

## TODO

- [x] When more features are added, consider moving `inscription`, `wallet`, and `indexer` into separate member crates in a workspace.

------------------------------------------------------------------------------------------------------------------------------

[![build-test](https://github.com/bitfinity-network/ord-rs/actions/workflows/build-test.yml/badge.svg)](https://github.com/bitfinity-network/ord-rs/actions/workflows/build-test.yml)

## Get started

## Examples

### Deploy

You can see the example in `examples/deploy.rs` to see how to deploy a BRC20 token:

```sh
cargo run --example deploy --
  -T <tick>
  -a <total-supply>
  -l <mint-limit>
  -p <WIF private key>
  -n <network>
  <tx_input_id:tx_input_index> <tx_input_id:tx_input_index> <tx_input_id:tx_input_index>
```

### Mint

You can see the example in `examples/mint.rs` to see how to mint BRC20 tokens to your address.

```sh
cargo run --example mint --
  -T <tick>
  -a <mint-amount>
  -p <WIF private key>
  -n <network>
  <tx_input_id:tx_input_index> <tx_input_id:tx_input_index> <tx_input_id:tx_input_index>
```

### Transfer

You can use the example in `examples/transfer.rs` to transfer BRC20 token to another address.

To transfer tokens run the following command:

```sh
cargo run --example transfer --
  -T <tick>
  -a <token amount>
  -p <WIF private key>
  -n <network>
  <tx_input_id:tx_input_index> <tx_input_id:tx_input_index> <tx_input_id:tx_input_index> 
```

An example:

```sh
cargo run --example transfer --
  -T ordi 
  -a 100 
  -p "xxxxx" 
  -n testnet 
  "b6d2f6ebbf791f58cf5c15ca7ef936dc5485b27360c5e10c55b0170cf7429468:1" "f9832ed4eaf8eb32f619fe0e24f6ab352a73c16ee456b03792f13c6329e6a1e4:1"
```

Then you need to send the UTXO to the actual recipient of the transaction.
Let's say the previous command returned this output:

`Reveal transaction broadcasted: a9d7b9b6062a3609e9526b46540c6702185e612a2936f6382bf3b910cdab5b8f`

Then to send the transfer to the recipient, run the following command:

```sh
cargo run --example send-inscription --
  -t tb1qg0707euju8jmjr0f2erdukcttwwc0lt7p4at93 
  -p "xxxx" 
  -n test 
  -i "a9d7b9b6062a3609e9526b46540c6702185e612a2936f6382bf3b910cdab5b8f:0" "0c86a1ba63234546c234a6e253a0844bb693d8093dc65a6cf28f200d475bd675:1"
```

Where `-i` takes the reveal transaction and then, the positional arguments are the transactions which will fund the fees for the transaction.

#### How transfers works

The transfer involves two steps actually, let's see an example where Alice sends 100 ordi to Bob:

1. First the Alice sends a commit transaction from her **source** address to a random P2TR derived from her
2. Then Alice reveals the inscription in the reveal transaction from the P2TR address to her **source** address.
3. Finally, Alice sends the UTXO from the reveal transaction to Bob's Address.

### Inscription parsing

In order to parse an inscription you can use the `OrdParser::parse` function, which will use the `parse` function from the `Inscription` trait, for the given Inscription type.

For example, given the transaction `ff314aebaa91a3f10cfba576d3be958127aba982d29146735e612869567e7808` from the testnet, we'll parse a valid `Brc20`.

```rust
let transaction = get_transaction_by_id(
    "ff314aebaa91a3f10cfba576d3be958127aba982d29146735e612869567e7808",
    bitcoin::Network::Testnet,
)
.await
.unwrap();

let inscription: Brc20 = OrdParser::parse(&transaction).unwrap().unwrap();
assert_eq!(inscription, Brc20::transfer("mona", 100));
```

## References

- [Ordinal Theory](https://docs.ordinals.com/inscriptions.html)
- [BRC-20 Standard](https://domo-2.gitbook.io/brc-20-experiment/)

## License

See license in [LICENSE](./LICENSE)
