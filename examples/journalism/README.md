# journalism

*A worked yidam corpus: what the reporting rests on, and what may leave the building.*

Eleven instances across four classes, three catalog sources, two decision records, one skill,
and a two-vault configuration. Small enough to read in ten minutes, and arranged around the
one condition that makes this domain different from the others here: **the newsroom may
publish a finding about a document it may not host.**

## Nothing here is real, and in this domain that line is drawn hard

**Ostreza Freight Holdings does not exist.** Neither does Lorne Vasquez, the State Transport
Board, the consent order, the inspection file, or the memo. Every finding in this corpus is
invented, about an invented company, and no real entity appears as a subject anywhere.

That is stated first, in this repository's own voice, because an example ships **to be
copied**. A corpus of invented findings attached to a real company's name is defamation-shaped
whatever the surrounding disclaimer says, and the copy of it somebody makes will keep the
structure and change the details. So the structure is what has to be safe.

The **conventions** are real, and they are the transferable part: the annual-report item
structure (Item 1 business, Item 1A risk factors, **Item 3 legal proceedings**, Item 7
management's discussion, Item 8 financial statements); records-request responses released *in
part* with each withholding cited to its exemption — **5 U.S.C. § 552(b)(4)** for confidential
commercial information and **§ 552(b)(6)** for personal privacy; and the fact that a released
public record may be republished while material provided under terms may not.

The artifact digests are real `sha256` values — each is the hash of its own short name, so
`printf 'edgar-item-3' > f && yidam vault put f` reproduces the cache state the walkthrough
shows, rather than a plausible-looking string that is the hash of nothing.

## The shape of it

```text
.yidam/
  config.toml                two vaults, two audiences
  corpus/
    entity.ont.yml    document.ont.yml           finding.ont.yml            thread.ont.yml
    entity/           document/                  finding/                   thread/
      ostreza-freight   annual-report-item-3       undisclosed-consent-order  regulatory-history
      lorne-vasquez     inspection-file-response   deferred-maintenance       terminal-conditions
      state-transport-  maintenance-memo           officer-tenure-overlap
        board
  catalog/edgar-filings.md
  catalog/transport-board-records.md
  catalog/confidential-material.md
  decisions/allegation-is-not-a-class.yml
  decisions/hosting-and-standing-are-separate.yml
  skills/assess-a-finding.md
```

## What each piece is here to demonstrate

**The class that was rejected.**
[`decisions/allegation-is-not-a-class.yml`](.yidam/decisions/allegation-is-not-a-class.yml):
an allegation with a document is a finding, and one without is a finding at `[open]`. A
separate class would be a place to put assertions with no provenance — and the nodes in it
would be exactly the ones most in need of the discipline the rest of the graph is under.

**Two facts about one document that must not be conflated.**
[`decisions/hosting-and-standing-are-separate.yml`](.yidam/decisions/hosting-and-standing-are-separate.yml).
Whether a document may be republished says nothing about how well it supports a finding.
`finding/deferred-maintenance` is where both apply at once, and its body says so in two
deliberate sentences.

**One entity class for sources and subjects.** `entity/state-transport-board` is both — the
board's own conduct is a live question *and* a third of the record came from it. Separating
sources from subjects would have put that node in neither class.

**A withholding is evidence.** `catalog/transport-board-records` records that the response
was released *in part* with its exemptions cited. A `(b)(4)` withholding says the agency
treated something as confidential commercial information, which is a fact about the record
that survives its contents being unavailable — and it is why `finding/deferred-maintenance` is
`[open]` rather than unsupported: there is a known place where the answer probably is.

**Two audiences, two stores.** [`config.toml`](.yidam/config.toml) declares a public store for
derived output and a newsroom store for source documents. `vault push --dry-run` refuses the
memo *by name*, under the audience of the store it was headed for.

## Running the gates

```sh
cp -R examples/journalism /tmp/journalism
cd /tmp/journalism && git init -q && git add -A && git commit -qm "chore: genesis — journalism"
yidam graph-check     # 11 instances across 4 classes — all clean
yidam lint            # 0 finding(s), no errors
yidam open-questions  # four live questions
yidam vault list      # two stores, their audiences, and what routes to each
```

The walkthrough is
[docs/walkthroughs/investigative-journalism.md](../../docs/walkthroughs/investigative-journalism.md).
