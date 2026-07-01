Elector or agent avatar. Human → circle with ma-rose tint. Agent → rounded square with rigpa-blue tint. Visual parity between participant types is intentional per the equality principle.

```jsx
<Avatar name="alice" variant="human" size="md" />
<Avatar name="agent-01" variant="agent" size="sm" />
<Avatar name="ma/bob" src="/avatars/bob.jpg" size="lg" />
```

**Initials logic:** "alice" → "AL" · "ma/bob" → "MA" (split on `/`) · "alice chen" → "AC"

**Never use color alone to distinguish participants.** The shape (circle vs rounded square) is the primary differentiator.
