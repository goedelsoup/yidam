Summary card for a single corpus node. Title uses `--font-display`, excerpt uses `--font-serif`. Interactive when `onClick` is supplied.

```jsx
<NodeCard
  path=".yidam/corpus/hydrology/watershed-boundaries.md"
  title="Watershed Boundaries"
  excerpt="The study watershed is bounded by the Continental Divide to the north and the state line to the south, encompassing 2,400 km²."
  outgoing={4} incoming={7}
  lineCount={18} lastModified="2 days ago"
  markers={['verified']}
  onClick={() => navigate(node.path)}
/>

<NodeCard
  path=".yidam/corpus/hydrology/?"
  title="What drives sediment transport seasonality?"
  markers={['open']}
  outgoing={2} incoming={1}
  isOpenQuestion
/>
```
