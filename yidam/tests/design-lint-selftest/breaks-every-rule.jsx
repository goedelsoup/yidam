// The file the adherence lint must report on.
//
// It lives here rather than under `yidam/design/` so the real run never sees it, and
// `scripts/design-lint-selftest.sh` points the linter at it and requires a non-zero exit.
//
// Why a fixture and not a reading of the config: for the whole of its life the config's rules
// were inert. Forty-seven `no-restricted-syntax` selectors named a rule oxlint does not
// implement — it is absent from `oxlint --rules` and an unknown key is accepted and ignored —
// and the one rule that is implemented was disabled everywhere by an `overrides` block meant
// to exempt `index.js`, against patterns that could not have matched a relative import
// anyway. `design_lint.rs` asserted the rules were present, were errors, and were invoked,
// and every one of those assertions was true of a lint that caught nothing.
//
// Reading a config cannot tell you whether a linter enforces it. Running it can.
import React from 'react';
import { Badge } from '../../design/components/core/Badge.jsx';

export function BreaksEveryRule() {
  return <Badge>{React.version}</Badge>;
}
