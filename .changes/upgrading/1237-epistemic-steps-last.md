### `yidam run` plans every epistemic step last

**A run that both lands and proposes now proposes at the head it leaves (#1237).**
Before, a proposal planned between two landings named a head the branch had already passed.
The next run found no `propose/<head>` for the new tip, and proposed again.

**What changes for you: nothing to do.** Dependencies still run first. A step's report may list in a new order.
