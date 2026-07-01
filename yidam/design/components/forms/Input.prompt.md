Text input with label, focus ring (gold), error state, and optional prefix/suffix decorators.

```jsx
<Input label="Node title" placeholder="e.g. watershed-boundaries" />
<Input label="Search" prefix={<SearchIcon />} placeholder="Search corpus…" />
<Input label="Commit hash" error="Must be a valid 7-char hash" value={val} onChange={e => setVal(e.target.value)} />
```

**States:** default · focused (gold ring) · error (red border + message) · disabled (muted)

**Prefix/suffix:** pass any ReactNode — icons, units, or text. Use Lucide icons at 14px with stroke 1.5.
