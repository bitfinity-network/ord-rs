# OrdinalsV2

Ordinal-aware Bitcoin wallet and indexer for Ordinal inscriptions.

## Modules

- [inscription](/src/ordinals_v2/inscription): `Brc20` and `Nft` inscription types.
- [wallet](/src/ordinals_v2/wallet): Transaction builder and processor.
- [indexer](/src/ordinals_v2/indexer): Scans the Bitcoin blockchain to catalog, organize, and provide information about Ordinal inscriptions.

## TODO

- [x] Consider moving `inscription`, `wallet`, and `indexer` into separate member crates in a workspace.
