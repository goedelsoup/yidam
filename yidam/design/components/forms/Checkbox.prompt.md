Custom checkbox with gold checked state. Manages its own hidden native input for accessibility.

```jsx
<Checkbox label="Commit as epistemic event" checked={isEpistemic} onChange={e => setIsEpistemic(e.target.checked)} />
<Checkbox label="Include generated nodes" checked={false} disabled />
```
