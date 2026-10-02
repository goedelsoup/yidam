### Source packs that read a paged answer to its end

**A scheme can declare `paginate` (#1341).**
Name the offset parameter, the record array, and how to know the last page.
`catalog-fetch` asks for every page, and records one artifact holding every record.
See [Source packs](source-packs.md#when-a-service-answers-in-pages).

**`catalog-fetch` refuses an answer with `exceededTransferLimit: true` from a scheme without `paginate`.**
The refusal is a finding on the entry, and the run fails.

**What changes for you: nothing, unless an entry fetched a cut-short answer.**
Such an entry recorded the first page only. Declare `paginate` on its scheme, and fetch it again.
