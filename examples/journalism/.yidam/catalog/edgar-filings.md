---
name: edgar-filings
description: The public filing system where the subject company's annual report is lodged.
type: database
obtained: true
retrieved: 2026-08-30
location:
  - kind: url_template
    value: https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK={cik}&type=10-K
    description: Company filing index by registrant identifier.
used-by:
  - ../corpus/entity/ostreza-freight.yml
  - ../corpus/entity/lorne-vasquez.yml
  - ../corpus/document/annual-report-item-3.yml
  - ../corpus/finding/undisclosed-consent-order.yml
artifacts:
  - sha256: 4135b43f2085935849297878c77b3250dc4c1bc8b19e921bd386610bab112155
    bytes: 12
    media_type: text/plain
    retrieved: 2026-08-30
    redistributable: true
---

# Public filings

An annual report on Form 10-K is filed with a regulator and published by it. The structure is
fixed and is worth knowing by item number, because it tells you where to look before you know
what you are looking for: **Item 1** business, **Item 1A** risk factors, **Item 3** legal
proceedings, **Item 7** management's discussion, **Item 8** the financial statements.

Item 3 is the one this reporting starts from. A registrant describes material pending legal
proceedings there, and what a registrant treats as material is itself a datum.

## What this corpus takes from it

The **conventions** — the item structure, and the fact that filings are public and hosted by
the receiving regulator, which is why the artifact here is `redistributable: true`.

The **registrant is invented.** There is no Ostreza Freight Holdings, no filing, and no
proceeding. See the corpus README.

## What it does not answer

**A filing is the registrant's account of itself.** It is a primary source for what the company
said and a weak one for what happened. An absence in Item 3 is evidence of what was disclosed
and not of what exists — which is exactly why `finding/undisclosed-consent-order` needs the
inspection file as well, and would be unsupportable on the filing alone.

**Materiality is the registrant's judgement.** The standard is not a threshold this corpus can
apply, so a finding that something *should* have appeared in Item 3 is a legal conclusion and
is out of scope here.
