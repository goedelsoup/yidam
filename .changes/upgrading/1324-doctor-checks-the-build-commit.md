### `doctor` checks that your binary was built at the pin

**`yidam doctor` has a new `commit` check (#1324).**
It compares the running binary's build commit with `.yidam.toml`'s `commit`.
They agree when one hash is a prefix of the other.

**What changes for you: a stale `.yidam/bin/yidam` now fails `doctor`.**
That is the binary a re-vendor leaves behind when nobody rebuilt it.
The finding names both commits. Run `mise run yidam-build` to repair it.

A repository with no `.yidam/bin/yidam` gets a warning instead, since another `yidam` answered.
A binary that records no build commit also warns, because nothing can be compared.
