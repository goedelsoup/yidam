### A node may declare a refusal, and `derive check` holds an argument to it

**`refuses:` is a named node field (#1053, RFC-0045).** It lists the sentences in which a node
declines an inference. A new `refusal-span-drift` lint check is an error when one quotes text
the node's prose no longer holds. A corpus that declares no refusals sees nothing new.

**`yidam derive check` reads artifacts under `[derive] paths`.** Without that key, it reads
nothing and passes. With it, every memo or dossier there needs a `reach:` and `cites:` spans.
It must also answer each declared refusal in a paragraph it cites.

SDK parity moves 0.16.0 → 0.17.0 and `yidam-core` 0.10.0 → 0.11.0, for the new field. An SDK
that predates it keeps `refuses:` in `extra`, where nothing reads it.
