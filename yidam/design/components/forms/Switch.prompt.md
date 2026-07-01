Toggle switch for binary settings. Gold track when on, ink-200 when off.

```jsx
<Switch label="Include generated nodes" checked={includeGenerated} onChange={e => setIncludeGenerated(e.target.checked)} />
<Switch label="Strict mode" checked={false} disabled />
```
