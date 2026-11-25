### `retrieve` answers by keyword when the model cannot load

**A missing model no longer fails the call (#1516).** Before, `retrieve` returned an error when
the embedding model was neither cached nor downloadable. It now answers by keyword search.
`degraded_reason` is the new value `model_unavailable`. The MCP contract is 0.36.0.

A client that switches on `degraded_reason` should handle the new value. It describes a call,
as `remote_unavailable` does. The server retries the load on every search.

**The model cache moved to the corpus root.** It was `.fastembed_cache` under the working
directory. It is now `.fastembed_cache` under the corpus root, unless `[index] cache_dir` or
`$FASTEMBED_CACHE_DIR` names another place. A server started from another directory read the
wrong cache and downloaded the model again. If your model sits elsewhere, set the key:

```toml
[index]
cache_dir = "/srv/models"
```

**Seed the cache offline with `yidam model fetch --from <dir>`.** Copy a cache from a machine
that can download, then pass it. The command copies the files and checks the model loads.

The download still uses rustls with the bundled Mozilla roots. A gateway that re-signs TLS with
a private root still fails it. Seed the cache instead.
