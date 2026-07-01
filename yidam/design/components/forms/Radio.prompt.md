Single radio button. Group with shared `name` attribute. Inner dot is gold when checked.

```jsx
<Radio name="phase" value="Investigation" label="Investigation" checked={phase === 'Investigation'} onChange={() => setPhase('Investigation')} />
<Radio name="phase" value="Synthesis"    label="Synthesis"    checked={phase === 'Synthesis'}    onChange={() => setPhase('Synthesis')} />
```
