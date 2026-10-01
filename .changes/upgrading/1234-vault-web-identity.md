### An S3 vault can assume a role by web identity

**A vault can use a role from IRSA instead of a stored key (#1234).** The `default` vault
assumes `AWS_ROLE_ARN` with the token in `AWS_WEB_IDENTITY_TOKEN_FILE`. Another vault names its
own with `YIDAM_VAULT_<NAME>_ROLE_ARN` and `…_WEB_IDENTITY_TOKEN_FILE`. It never inherits
`AWS_ROLE_ARN`. The remote index reads `YIDAM_INDEX_ROLE_ARN` the same way. See
[cli-reference.md](cli-reference.md#s3-compatible-stores) and
[cluster-runs.md](cluster-runs.md#two-vault-backends).

**What changes for you: nothing, unless you opt in.** Keys you export for a vault still win over
an injected role. A vault given both its own role and its own keys is now refused. `doctor` now
says where each vault's credentials come from.

**Fenced clusters need STS.** Pods that assume a role call STS first. Add its interface
endpoint's addresses to `[cluster.egress] sts`, then regenerate the policies.

**Update the image first.** An older binary refuses `.yidam/config.toml` once it holds
`[cluster.egress] sts`.
