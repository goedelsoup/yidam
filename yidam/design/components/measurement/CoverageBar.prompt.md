Line coverage as a bar with three states — covered, uncovered, and **unmeasured**.

```jsx
<CoverageBar label="This change" covered={41} uncovered={7} unmeasured={0} features={['reports']} />
<CoverageBar label="This change" covered={0} uncovered={0} unmeasured={128} features={['reports']} />
```

**Unmeasured is not a coverage gap.** It is lines in files the build did not compile — a
statement about the build, not about the tests. It is drawn in the neutral ink family, is
excluded from the percentage, and the second example above reads *"not measured"* rather than
`0%`.

**`features` is required.** A coverage number whose build is unstated cannot be read: a pull
request compiles the light default, so the feature-gated paths are absent from the measurement
entirely. The component prints the feature set beneath the bar whether or not anything was
unmeasured.
