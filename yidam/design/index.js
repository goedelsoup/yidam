// The design system's entry point.
//
// `_adherence.oxlintrc.json` has forbidden importing `components/<group>/**` since it was
// written, with the message "Import design-system components from 'index.js', not component
// internals." There was no `index.js`. Nothing imported anything either, so the rule had
// never fired and the file it named had never been missed — the same shape as the lint that
// nothing invoked (#465) and the verification task that had never run (#461), one level in.
//
// #467's quality pages are the system's first consumer. This is the door they come through.
//
// `_ds_bundle.js` is not this. It is a generated browser IIFE that hangs the components off
// `window.YidamDesignSystem_76df35` for the design tool that produced it, and it carries
// source hashes rather than exports. `design_system.rs` asserts this file and the manifest
// name the same components, so a component added to one and not the other is a red build
// rather than an import that resolves to `undefined` at render time.

export { Avatar } from './components/core/Avatar.jsx';
export { Badge } from './components/core/Badge.jsx';
export { Button } from './components/core/Button.jsx';
export { Card } from './components/core/Card.jsx';
export { Tag } from './components/core/Tag.jsx';

export { Dialog } from './components/feedback/Dialog.jsx';
export { Toast } from './components/feedback/Toast.jsx';

export { Checkbox } from './components/forms/Checkbox.jsx';
export { Input } from './components/forms/Input.jsx';
export { Radio } from './components/forms/Radio.jsx';
export { Select } from './components/forms/Select.jsx';
export { Switch } from './components/forms/Switch.jsx';
export { Textarea } from './components/forms/Textarea.jsx';

export { BranchRef } from './components/knowledge/BranchRef.jsx';
export { ClaimMarker } from './components/knowledge/ClaimMarker.jsx';
export { NodeCard } from './components/knowledge/NodeCard.jsx';
export { PhaseTag } from './components/knowledge/PhaseTag.jsx';

export { CoverageBar } from './components/measurement/CoverageBar.jsx';
export { StatusMeter } from './components/measurement/StatusMeter.jsx';

export { Breadcrumb } from './components/navigation/Breadcrumb.jsx';
export { Tabs } from './components/navigation/Tabs.jsx';
export { Tooltip } from './components/navigation/Tooltip.jsx';
