Raised content container with white background, subtle border, and small radius.

```jsx
<Card padding="md">
  <p>Node content goes here.</p>
</Card>

<Card padding="sm" onClick={() => navigate(node.path)}>
  <strong>{node.title}</strong>
</Card>
```

**Interactive card:** pass `onClick` to enable hover state (border strengthens from ink-200 → ink-300).

**No colored left-border accent.** No heavy shadows. Keep decoration minimal — the content is the signal.
