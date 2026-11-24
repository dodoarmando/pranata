# Pranata

Pranata is a village website and administration product for many Indonesian villages: a public site for residents plus an admin area where village staff run the village's records and site content. Each village runs its own Installation.

This glossary names the domain. Code, tests and documents use these terms; the Indonesian words appear only in the UI.

## Language

### Deployment

**Installation**:
One running deployment of Pranata on its own VPS, serving exactly one Village with its own database.
_Avoid_: Tenant, instance, site

**Village**:
The single Indonesian village (desa) an Installation belongs to and publishes for.
_Avoid_: Desa (in code), tenant, client, organisation

**Setup Wizard**:
The first-run flow in which the first Administrator enters the Village Code and the Village's settings; an Installation without a completed Setup Wizard serves nothing else.
_Avoid_: Onboarding, install script

### Village

**Village Code**:
The Village's 10-digit government region code (Kepmendagri), which fixes its name, its district, regency and province, and its time zone from a dataset bundled with Pranata.
_Avoid_: Kode desa (in code), village ID

**Village Settings**:
The Village's own choices that are not fixed by its Village Code: logo, primary colour, Hamlet naming, office hours, contact details, social links, and the optional external service URL.
_Avoid_: Config, preferences

**Vision and Mission**:
The Village's current vision statement and ordered missions, together with the RPJMDes period they belong to.
_Avoid_: Visi misi (in code), goals

**Region Unit**:
Any node of the Village's internal region tree: a Hamlet, an RW, or an RT. It is deactivated, never deleted, once in use.
_Avoid_: Area, zone, wilayah (in code)

**Hamlet**:
The top-level Region Unit below the Village, called dusun by default and by a Village-chosen name elsewhere (banjar, jorong, lingkungan).
_Avoid_: Dusun (in code), district, sub-village

**RW**:
The optional middle Region Unit, belonging to one Hamlet and numbered uniquely across the Village.
_Avoid_: Rukun warga (in code), block

**RT**:
The lowest Region Unit, belonging to one RW (or to one Hamlet when the Village has no RW level) and numbered uniquely within it.
_Avoid_: Rukun tetangga (in code), neighbourhood

**Position**:
A post in the Village government that Officials hold, such as village head, secretary, or the head of one Hamlet.
_Avoid_: Jabatan (in code), role, title

**Tenure**:
One period during which an Official holds a Position, either permanently (definitif), as a temporary appointee (Pj.), or as an acting holder (Plt.); a Position has at most one active Tenure, and Tenures are never deleted.
_Avoid_: Term, assignment, masa jabatan (in code)

**Official**:
A person holding or having held a Tenure in the Village government (perangkat desa). An Official need not be a User.
_Avoid_: Staff, officer, perangkat (in code)

**Institution**:
A body outside the Village government (BPD, LPM, PKK, BUMDes and the like) with an ordered list of members, none of whom sign letters.
_Avoid_: Organisation, lembaga (in code)

**Indeks Desa Edition**:
The ministry's score, status and dimension scores for the Village for one data-collection year, on either the Indeks Desa scale (0–100) or the older IDM scale (0–1).
_Avoid_: Ranking, IDM score (for current editions)

### Website

**Draft**:
The state of a website item that is saved but not shown on the public site.
_Avoid_: Unpublished, pending

**Submitted**:
The state of a Draft that its author, who cannot publish, has handed in for someone with the publish Permission to review.
_Avoid_: Pending, in review, awaiting approval

**Published**:
The state of a website item that is shown on the public site; unpublishing returns it to Draft.
_Avoid_: Live, active, terbit (in code)

**Slug**:
The URL name of a website item, fixed when the item is first Published; a later explicit change keeps the old Slug as a redirect.
_Avoid_: Permalink, path

**News Post**:
A dated report on the public site, filed under one Category.
_Avoid_: Post, article, berita (in code)

**Announcement**:
A notice with a start and end date, shown on the home page only while it is in effect.
_Avoid_: Notice, pengumuman (in code)

**Event**:
A scheduled village activity with a start and end time, location and Category, which may be cancelled while staying listed.
_Avoid_: Activity, agenda (in code)

**Page**:
Undated free-form content placed under one of the public site's fixed parent menus.
_Avoid_: Static page, halaman (in code)

**Album**:
An ordered set of photos, each with an optional caption, from one village occasion.
_Avoid_: Gallery (for a single album), galeri (in code)

**Important Contact**:
A phone number residents may need, with a short description, grouped by Contact Category.
_Avoid_: Contact, kontak penting (in code)

**Contact Category**:
A Village-defined, ordered label that Important Contacts are grouped under on the public site; one in use is kept.
_Avoid_: Category (reserved for News Posts and Events), kategori kontak (in code)

**Partner**:
An organisation whose logo appears, in a Village-chosen order, in the home page partner strip.
_Avoid_: Sponsor, mitra (in code)

**Transparency Document**:
An official planning, budget or regulation file published for download, tagged with a document type and fiscal year.
_Avoid_: Attachment, file, dokumen (in code)

**APBDes Summary**:
The public summary of one fiscal year's village budget: its version and adoption date, income per source, spending per bidang, financing, and optionally its realisation per semester; totals are computed, amounts are whole rupiah.
_Avoid_: Budget, anggaran (in code)

**Category**:
A Village-defined label shared by News Posts and Events, with a built-in default.
_Avoid_: Tag, topic, kategori (in code)

**Quick Link**:
A Village-chosen shortcut to a page of the public site, shown under the home page search box.
_Avoid_: Popular search, tag, tautan cepat (in code)

**Home Hero**:
The photo, heading, lead text and button at the top of the home page.
_Avoid_: Banner, slider

**Head's Welcome**:
The village head's welcome text and portrait, signed by the active Tenure of the village head Position.
_Avoid_: Greeting, sambutan (in code)

**Media Asset**:
An uploaded image or document, stored once with its metadata stripped, from which display variants are derived.
_Avoid_: File, upload, attachment

### Access

**Permission**:
One action the system allows or denies, drawn from a fixed catalogue defined by Pranata itself.
_Avoid_: Right, privilege, hak akses (in code)

**Role**:
A named set of Permissions that a Village defines for itself.
_Avoid_: Group, peran (in code)

**Administrator**:
The built-in Role that always holds every Permission and cannot be edited; an Installation always has at least one active User with it.
_Avoid_: Admin, superuser, root

**User**:
A person who signs in to the admin area on behalf of the Village with a username that is never changed or reused, holding exactly one Role; never deleted, only deactivated.
_Avoid_: Account, staff, operator, pengguna (in code)

**Session**:
A signed-in User's server-side login, identified by a cookie.
_Avoid_: Token, login

**Sudo Mode**:
A short window after re-authentication during which a User may perform sensitive actions.
_Avoid_: Elevated mode, admin mode

**Passkey**:
A WebAuthn credential a User registers to sign in without a password.
_Avoid_: Security key, biometric login

**Access Link**:
A single-use, short-lived link an Administrator (or the server CLI, for the first Administrator) hands to a User so they can set their own password, used both for a new User and to restore a User's access.
_Avoid_: Invite, reset link, magic link, temporary password

**Audit Entry**:
One immutable record of who did what to which object, when, and what changed.
_Avoid_: Activity, history, event

**Audit Log**:
The complete, append-only collection of Audit Entries for an Installation.
_Avoid_: Activity log, history

## Reserved for later releases

These terms are defined in the archived attempt and will be restated here when their release is designed: Resident, NIK, Family, Population Event (Birth, Death, Arrival, Departure), Baseline, Monthly Recap, Sent, Resident Register, Mutation Register, Temporary Stay (R2); Letter Type, Layout, Service Letter, Recorded Letter, Letter Number, Issued, Cancelled, Verification Code, Outgoing Letter Agenda (R3); Letter Request, Complaint (R4); Retention Schedule, Destruction (R2).
