### The SDKs are no longer vendored

**The SDKs moved from `yidam/prelude/sdks/` to `yidam/sdks/` (#1311).** The prelude copy no longer carries them.
The next `yidam-vendor-update` deletes `.yidam/.vendor/prelude/sdks/`, which held 66 to 377 files.
It does so even when the pinned ref predates the move.

**What changes for you: probably nothing.** No derived repository we know of builds the vendored SDKs.
If yours depends on one by path, depend on the published `yidam-core` crate instead.
Its manifest now sits at `yidam/sdks/rust/Cargo.toml` upstream.
