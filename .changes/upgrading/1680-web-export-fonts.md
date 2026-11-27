### An exported web page no longer asks Google for its fonts

**`export --format web` writes a stylesheet that requests nothing over the network (#1680).**

Before, the first rule of `assets/main.css` imported four font families from Google Fonts.
Every reader of a published page made that request on open, and no config named it.
The page now uses those families where the reader has them installed.
Elsewhere it falls back to Georgia, the system sans and Courier New, as it already did offline.
Re-export a published page to pick this up; the command and its flags are unchanged.
The module URLs under `cdn` in `web.config.json` are unchanged, and are still the page's only remote loads.
