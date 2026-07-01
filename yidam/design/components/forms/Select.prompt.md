Styled native select. Pass options as strings or `{ value, label }` objects.

```jsx
<Select label="Phase type" value={phase} onChange={e => setPhase(e.target.value)}
  options={['Investigation', 'Extraction', 'Synthesis', 'Assessment']} />

<Select label="Branch" placeholder="Select a branch…"
  options={branches.map(b => ({ value: b, label: b }))} />
```

**Native select** is used intentionally: keyboard nav, screen readers, and form submission all work without custom JS. Style overrides are via appearance:none + custom chevron.
