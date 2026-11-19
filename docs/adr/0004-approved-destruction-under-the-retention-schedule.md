# Approved Destruction under the Retention Schedule

Pranata never erases records automatically. Each kind of record has a keeping period in the Village's Retention Schedule, taken from its regency's archive retention schedule (JRA). When records pass that period, a User with `record.destroy` runs a Destruction: the records due are listed, the destruction report (berita acara) number, date and approving official are entered, the personal fields are erased, the statistical skeleton is kept, and the act is written to the Audit Log.

Defaults until a Village enters its own JRA:

- Residents, Families, Population Events and Temporary Stays: permanent, never destroyed in the product.
- Issued Service Letters and the Outgoing Letter Agenda: 2 + 3 years after the agenda year closes.
- Letter Requests and Complaints: 2 + 3 years after their final state.
- Audit Log: 5 years. Audit Entries are destroyed only through a server CLI that uses a separate database role, run by the vendor on the Village's written instruction, so the application's role stays `INSERT`/`SELECT` only.

Technical logs (journald, 30 days) and backup copies (48 hourly, 30 daily, 12 monthly) still rotate automatically. They are operational copies, not the records themselves.

The reason is archive law: UU 43/2009 Art. 51 and 86 allow records to be destroyed only under a retention schedule, with criminal penalties otherwise, and UU 27/2022 Art. 44 ties the destruction of personal data to the same schedule.

We rejected two alternatives:

- Automatic erasure 90 days after a Letter Request or Complaint closes, which earlier decisions chose. It destroys records outside the JRA procedure.
- Keeping everything with no Destruction at all. It leaves Villages unable to follow their own JRA once records come due.

Personal-data deletion requests against permanent population records are answered by correction, not erasure.
