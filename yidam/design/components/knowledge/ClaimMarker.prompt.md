Inline epistemic claim marker. The single most brand-defining component in yidam.

```jsx
// Inline within prose — use size-relative em units
<p>
  Base flow in the watershed <ClaimMarker type="verified" /> is between 0.3 and 1.2 m³/s.
  Peak flows <ClaimMarker type="inference" annotation="based on NWIS gauge 09380000" /> occur in spring.
  Whether snowmelt drives peak timing <ClaimMarker type="open" /> remains under investigation.
</p>
```

**Types:**
- `verified` — green; primary source committed
- `inference` — amber; reasonable conclusion from verified facts
- `open` — blue; live question or contested claim

**Do not use Badge for epistemic annotation** — ClaimMarker uses monospace font and bracket notation specifically to signal epistemic intent.
