Transient notification toast. Position it in a fixed-bottom container in your app shell.

```jsx
<Toast type="success" message="Epistemic commit recorded" detail="3 nodes updated · genesis@a3f2b1c" onDismiss={() => clearToast()} />
<Toast type="info" message="Index refresh in progress" />
<Toast type="error" message="Connector failed" detail="nwis: upstream timeout" onDismiss={() => {}} />
```

**Types:** `info` (dark) · `success` (green) · `error` (red) · `warning` (gold-dark)
