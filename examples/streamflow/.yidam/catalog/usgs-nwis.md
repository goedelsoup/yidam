---
name: usgs-nwis
description: USGS National Water Information System — continuous streamflow records for United States gaging stations.
type: api
obtained: true
location:
  - kind: url
    value: https://waterdata.usgs.gov/nwis
    description: Human-facing query interface.
  - kind: url_template
    value: https://waterservices.usgs.gov/nwis/iv/?sites={site}&parameterCd=00060&format=json
    description: Instantaneous-values service; parameter 00060 is discharge.
used-by:
  - ../corpus/gage/canyon-outlet.yml
  - ../corpus/gage/valley-bridge.yml
---

# USGS NWIS

The system of record for continuous streamflow in the United States. Discharge is published
under **parameter code 00060**, in cubic feet per second, alongside gage height (00065).
Instantaneous values are typically at 15-minute intervals; daily values are a separate
service.

## What this corpus takes from it

The **conventions**, not the data. The gage nodes here are illustrative stations, and they
carry NWIS's parameter code and units so that the shape of a real record is legible. No
observation from NWIS is reproduced in this corpus.

That is why no node here cites this source at `[verified]` for a *value*. It is cited for
what a discharge record is and how it is denominated — which is exactly what this source is
the authority on.

## What it does not answer

NWIS publishes the record. It does not publish the **rating curve** that produced it, nor
the operating rule upstream of a regulated station. Both are the substance of most of the
questions this corpus is open on, and neither can be recovered from the discharge series.

Provisional values are flagged and are revised, sometimes substantially, after review. A
figure retrieved from the instantaneous service is not final and should not be tagged as
though it were.
