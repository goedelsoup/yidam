---
name: transport-board-records
description: Inspection records released by the State Transport Board in response to a records request, in part.
type: other
obtained: true
retrieved: 2026-08-28
location:
  - kind: address
    value: State Transport Board, records office
    description: Request filed and answered by post; the response letter enumerates the withholdings.
used-by:
  - ../corpus/entity/state-transport-board.yml
  - ../corpus/document/inspection-file-response.yml
  - ../corpus/finding/undisclosed-consent-order.yml
artifacts:
  - sha256: 576345cde063d82ba9a1e0c3b8e6563a0f72a8fe3052f70a2187c8b4cdf2788d
    bytes: 20
    media_type: text/plain
    retrieved: 2026-08-28
    redistributable: true
---

# Records-request response

Released **in part**. The response letter enumerates what was withheld and cites the exemption
for each — the citation form is the useful convention here, because an exemption is a claim the
agency is making and it can be appealed on its own terms.

Two exemptions are cited: **5 U.S.C. § 552(b)(4)**, commercial or financial information obtained
from a person and privileged or confidential, and **§ 552(b)(6)**, personnel and similar files
whose disclosure would be a clearly unwarranted invasion of personal privacy.

## What this corpus takes from it

The **conventions** — that a response is released in part rather than granted or denied, that
each withholding is cited to an exemption, and that a released record is public and therefore
`redistributable: true`.

The agency, the request and the records are **invented**.

## What it does not answer

**A withholding is not an absence.** A (b)(4) citation on an inspection record says the agency
treated something in it as confidential commercial information. That is a fact about the record
that survives the contents being unavailable, and it is why `finding/deferred-maintenance` is
`[open]` rather than unsupported — there is a known place where the answer probably is.

**The response describes what was released, not what exists.** A request is scoped by its own
terms, and records outside the scope are not withheld, they are simply not searched for. The
distinction matters when reading a response as evidence of absence: this one cannot support
"there were no other inspections", only "none were released under this request".
