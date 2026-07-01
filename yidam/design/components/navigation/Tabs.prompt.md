Tab navigation bar. `underline` variant uses a gold bottom-border indicator; `pill` variant renders a segmented-control style on a muted background.

```jsx
const tabs = [
  { id: 'corpus', label: 'Corpus', count: 24 },
  { id: 'catalog', label: 'Catalog', count: 8 },
  { id: 'open', label: 'Open questions', count: 3 },
];
<Tabs tabs={tabs} activeTab="corpus" onChange={setTab} />
<Tabs tabs={tabs} activeTab="corpus" onChange={setTab} variant="pill" size="sm" />
```

**With content panel:**
```jsx
<Tabs tabs={tabs} activeTab={tab} onChange={setTab}>
  {tab === 'corpus' && <CorpusList />}
</Tabs>
```
