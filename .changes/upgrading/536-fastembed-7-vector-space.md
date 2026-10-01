### Every vector index needs rebuilding

**The embedding runtime moved to fastembed 7, and it embeds into a different vector space (#536).**
Its ONNX Runtime is newer, and the quantized kernels answer about 3e-4 differently per dimension.
The TypeScript SDK moved with it, because transformers.js 4.3 answers the same way.

**What changes for you: re-run `yidam index-build`.** Re-run `yidam index-push` too if you push one.
Until then, semantic `retrieve` reports `stale_contract` and answers with keyword search instead.
It does not rank against vectors from the old space. An index built before the witness existed is not checked.
`yidam index-verify` reports `mismatch` on an old index, and `match` once it is rebuilt.
