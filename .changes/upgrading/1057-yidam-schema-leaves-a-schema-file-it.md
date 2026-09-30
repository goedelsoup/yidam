### `yidam schema` leaves a schema file it did not write alone

**Repo-owned schemas (#1057).** Every file `yidam schema` writes now carries a marker in its
`$comment`. When a target in `.yidam/schemas/` has no marker, the run refuses before writing
anything, and names the files. A repository that compiles its own, stricter schemas into that
directory keeps them. It needs no guard task around `mise run schema` any more.

Files an earlier release wrote have no marker either. The first run after this upgrade
refuses them. Replace them once, and commit the result:

```sh
yidam schema --force
```

`yidam schema --settings` is unchanged. It reads no schema file and writes none.
