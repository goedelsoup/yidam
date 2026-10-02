### A kuten profile with a populated `policy` slot is refused

**A kuten proposes no severities, and a profile that tries is no longer read (#1305).**
The `policy` slot was parsed and then ignored, so a proposed severity reached nothing.
The policy layer decides disclosure only. Nothing in it changes a check's severity.

**What changes for you: nothing, unless you author a kuten profile.**
Every shipped profile carries `proposes_overrides: []`, which still parses.
A non-empty list now stops `kuten check` with an error, and `doctor` warns. Both name the slot.
