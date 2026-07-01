Removable label chip for editable collections. Pill-shaped; renders an × remove button when `onRemove` is provided.

```jsx
<Tag onRemove={() => removeClass('watershed')}>watershed</Tag>
<Tag variant="rigpa" onRemove={() => {}}>rigpa/first-synthesis</Tag>
<Tag variant="gold">axiom</Tag>
```

**When to use Tag vs Badge:** Tag is for user-editable lists (remove action available). Badge is for read-only metadata labels.
