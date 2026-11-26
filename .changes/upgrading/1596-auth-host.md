### A pack's credential names the host it is sent to

**An `auth` entry takes `host`, and is sent only on a request to that host (#1596).**

`source check` now refuses a pack reaching two or more hosts with an unscoped credential.
Add `host = "<the publisher's host>"` to each entry in such a pack.
An unset scoped variable leaves the pack ready, and only requests to that host are refused.
See [Source packs](source-packs.md).
