### Domain libraries are vendored beside the prelude, not inside it

**The domain libraries moved from `yidam/prelude/domains/` to `yidam/domains/` (#934).** The prelude copy no longer carries them.
`yidam-vendor-update` copies only the domains named in `prelude_domains` into `.yidam/.vendor/domains/`.
Without that key, it keeps the domains this repository already vendored.

**What changes for you: a vendored domain changes path.** The next re-vendor moves it from `.yidam/.vendor/prelude/domains/<domain>/` to `.yidam/.vendor/domains/<domain>/`.
Re-point any `crates/Cargo.toml` path or `packages/` dependency that names the old path.
A repository that vendors no domain sees no change.
