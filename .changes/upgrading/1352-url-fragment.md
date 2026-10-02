### `catalog-location-malformed` reports a `url` with a `#` fragment

**A `url` location that names a member with `#` is now a finding (#1352).**
A fragment never reaches the server, so `catalog-fetch` downloads the whole file.
The entry then reads as if it held only the part the fragment names.
The finding names the file actually fetched.

**What changes for you: a corpus holding such a location gets a new warning.**
Move the part after `#` under `members:` to read one file inside a zip.
See [Read a file inside a zip](source-packs.md#read-a-file-inside-a-zip).
