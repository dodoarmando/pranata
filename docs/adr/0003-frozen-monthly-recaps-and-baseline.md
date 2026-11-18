# Frozen Monthly Recaps and a locked Baseline

A Monthly Recap marked Sent is stored as a frozen snapshot, together with that month's Mutation Register. It is never recalculated, and the mark cannot be removed. A Population Event recorded or voided later, but dated in a month already Sent, is counted in the first month not yet Sent. Its real date is kept and shown in the notes column.

Spreadsheet import may create Residents and Families only until the Village locks its Baseline. After that, import may only update fields of Residents it matches by NIK, and new people enter through Births and Arrivals.

Both rules keep one invariant that Permendagri 47/2016 book B.3 depends on: each month's opening balance equals the previous month's closing balance, and a figure already reported to the district never changes.

We rejected two alternatives:

- Always recalculating recaps from events, which the prototype did. It silently changes reported figures.
- Forbidding events dated in a Sent month. Villages often record births and deaths late.
