Action button for yidam surfaces. Renders a `<button>` with hover/press states tracked via React state.

```jsx
<Button variant="primary" size="md">Commit epistemic</Button>
<Button variant="ghost" size="sm" onClick={handleCancel}>Discard</Button>
<Button variant="danger" size="md" disabled>Delete node</Button>
```

**Variants:** `primary` (gold fill) · `ghost` (outlined) · `subtle` (transparent) · `danger` (red fill)

**Sizes:** `sm` 30px · `md` 36px · `lg` 42px

**Notes:**
- Always pair a primary with no more than one ghost on the same surface
- Use `subtle` for toolbar icon-adjacent text actions
- Focus ring is native; do not suppress it
