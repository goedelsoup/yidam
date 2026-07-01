Modal dialog. Renders `null` when `open` is false. Title uses `--font-display`. Backdrop blur on overlay.

```jsx
<Dialog open={open} onClose={() => setOpen(false)} title="Commit epistemic change"
  footer={<><Button variant="ghost" onClick={() => setOpen(false)}>Cancel</Button><Button>Commit</Button></>}>
  <Textarea label="Commit message" rows={4} placeholder="What changed in the world of knowledge?" />
</Dialog>
```

**Important:** manage `open` state externally. Dialog does not manage its own visibility.
