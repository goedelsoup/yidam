### Semantic retrieval is in every released binary

**`vector-read` joined the default feature set (#442).** Every install channel now carries `index-build` and semantic `retrieve`.
`yidam --version` lists `vector-read index` among its features.
The binary grows from about 23 MB to 56 MB.

**What changes for you: `retrieve` turns semantic once the corpus has an index.**
Run `yidam embed && yidam index-build`, or `yidam vault pull --index` where a vault holds one.
A corpus with no index answers by keyword, as before.
Semantic retrieval does not span `tonpa` dependencies; keyword retrieval does.

**On an Intel Mac, install ONNX Runtime 1.24 or newer.** `brew install onnxruntime` provides it.
The Homebrew tap installs it for you.
Elsewhere set `ORT_DYLIB_PATH` to a `libonnxruntime.dylib`.
Without it, only commands that embed fail, and each names the fix.

**To keep the old build, install from source.** Pass `--no-default-features` with the other default features.
[Installation](installation.md#check-which-build-you-have) has the command.
