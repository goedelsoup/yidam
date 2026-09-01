A series as a shape. Direction, not magnitude.

```jsx
<Sparkline label="Tests asserting" points={[1381, 1394, 1401]} />
<Sparkline label="Test seconds" points={[68.2, 71.0, 74.9]} higherIsWorse format={(v) => `${v.toFixed(1)}s`} />
```

No axes, no gridlines, no tooltip. The question a sparkline answers is *which way has this
been going*; everything else on a chart answers a different question that the series file
answers better.

**Fewer than two points is a stated absence**, not a flat line at zero — the same rule
`CoverageBar` follows. One measurement is not a trend.

**`higherIsWorse` picks the stroke colour and nothing else.** Test seconds going up is a
regression; tests asserting going up is not, and a line drawn in the same colour for both
teaches a reader to stop reading the colour.
