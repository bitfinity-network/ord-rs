# ord-rs Rust

[![build-test](https://github.com/bitfinity-network/ord-rs-rs/actions/workflows/build-test.yml/badge.svg)](https://github.com/bitfinity-network/ord-rs-rs/actions/workflows/build-test.yml)

## Get started

## Examples

### Transfer

You can use the example in `examples/transfer.rs` to transfer BRC20 token to another address.

To transfer tokens run the following command:

```sh
cargo run --example transfer --
  -t <recipient address of the transfer>
  -T <tick>
  -a <token amount>
  -p <WIF private key>
  -n <network>
  <tx_input_id:tx_input_index> <tx_input_id:tx_input_index> <tx_input_id:tx_input_index> 
```

An example:

```sh
cargo run --example transfer -- -t tb1qax89amll2uas5k92tmuc8rdccmqddqw94vrr86 
  -T ordi 
  -a 100 
  -p "xxxxx" 
  -n testnet 
  "b6d2f6ebbf791f58cf5c15ca7ef936dc5485b27360c5e10c55b0170cf7429468:1" "f9832ed4eaf8eb32f619fe0e24f6ab352a73c16ee456b03792f13c6329e6a1e4:1"
```

## License

See license in [LICENSE](./LICENSE)
