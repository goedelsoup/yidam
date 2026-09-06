A test run's outcome as a proportional bar, over `asserted` / `failed` / `skipped`.

```jsx
<StatusMeter label="ci (cli)" asserted={1397} failed={0} skipped={23} />
<StatusMeter label="yidam::vault_s3" asserted={0} failed={0} skipped={2} />
```

**The pass segment is `asserted`, not `passed`.** A runtime skip is recorded by a test runner
as a pass — the process ran and did not fail — so a suite that announced two skips and
exercised nothing carries the same `passed` as a suite that exercised everything. `asserted`
is `passed` minus the skips among it, it is a required prop, and the second example above
renders as an amber bar reading *"Ran and asserted nothing."* rather than as a green one.

**Colors:** jade for asserted, crimson for failed, saffron for skipped, via the
`--run-*` semantic tokens. Not the `verified` / `inference` / `open` triple: those are an
epistemic axis about what a corpus claims, and a passing test is not a verified claim.
