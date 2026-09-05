# docs

*Documentation for the yidam template itself — how it works, how to use it, and the decisions
that shaped it.*

This directory is part of the yidam template and does **not** get copied into derived
repositories. It describes yidam, not any domain being bootstrapped. For the documentation
scaffold that derived repos receive, see [sadhana/docs/](../sadhana/docs/README.md).

Everything below is published at **[goedelsoup.github.io/yidam](https://goedelsoup.github.io/yidam/)**,
rendered from these files on every push to `main`. This page is the exception: the site's
sidebar is its version of the contents table, so publishing it would ship a second copy.

## Contents

The sidebar in [`yidam/web/docs/astro.config.mjs`](../yidam/web/docs/astro.config.mjs) is the
contents table, and the [published site](https://goedelsoup.github.io/yidam/) is how to read it.
It is gated in both directions: a page missing from the sidebar fails the build, and a sidebar
entry naming no page fails too.

This file used to carry a second copy of that list. Nothing gated it, so it drifted — by the
time it was removed it had lost `artifact-vaults`, `upgrading`, five of six walkthroughs and
every research page. One contents table, and it is the gated one. The
[style guide](style-guide.md) records the rule.

## What belongs here

- Design documents for yidam features or architectural decisions
- ADRs (architecture decision records) for template-level choices
- Notes on the relationship between yidam layers (prelude, sadhana, samudaya, tools)
- Anything describing the template's own structure, not any derived repo's domain

What does **not** belong here: corpus nodes, domain knowledge, derived-repo documentation.
Those live in `corpus/`, the domain's `docs/`, or `sadhana/docs/` respectively.

New pages must be written to the [style guide](style-guide.md) and added to the sidebar, or the
docs build fails.
