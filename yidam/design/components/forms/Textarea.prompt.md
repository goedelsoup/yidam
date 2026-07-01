Multi-line text area, using --font-serif for a writing-oriented feel. Vertically resizable.

```jsx
<Textarea label="Commit message" rows={4} placeholder="What changed in the world of knowledge?" />
<Textarea label="Node excerpt" helper="2–10 sentences; one concept only" rows={6} />
```

**Note:** The serif font family signals that this field expects prose, not a path or code value. For code or path input, use Input with font-family override.
