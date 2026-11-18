# Pranata

Pranata is a village website and administration product sold to many Indonesian villages: a public site for residents plus an admin area where village staff run the village's records, letters, and site content.

## Language

### Deployment

**Installation**:
One running deployment of Pranata, serving exactly one Village with its own database.
_Avoid_: Tenant, instance, site

**Village**:
The single Indonesian village (desa) an Installation belongs to and publishes for.
_Avoid_: Desa (in code), tenant, client, organization

### Access

**Permission**:
One action the system allows or denies, drawn from a fixed catalog defined by Pranata itself.
_Avoid_: Right, privilege, hak akses (in code)

**Role**:
A named set of Permissions that a Village defines for itself.
_Avoid_: Group, peran (in code)

**Administrator**:
The built-in Role that always holds every Permission and alone manages Users and Roles.
_Avoid_: Admin, superuser, root

**User**:
A person who signs in to the admin area on behalf of the Village with a username that is never changed or reused, holding exactly one Role; never deleted, only deactivated. Residents are not Users.
_Avoid_: Account, staff, operator, pengguna (in code)

**Access Link**:
A single-use, short-lived link an Administrator (or the vendor, for the first Administrator) hands to a User so they can set their own password, used both for a new User and to reset a User's access.
_Avoid_: Invite, reset link, magic link, temporary password

**Audit Entry**:
One immutable record of who did what to which object, when, and what changed.
_Avoid_: Activity, history, event

**Audit Log**:
The complete, append-only collection of Audit Entries for an Installation.
_Avoid_: Activity log, history

### Website

**Draft**:
The state of a website item that is saved but not shown on the public site.
_Avoid_: Unpublished, pending

**Published**:
The state of a website item that is shown on the public site.
_Avoid_: Live, active, terbit (in code)

**News Post**:
A dated report on the public site, filed under one Category.
_Avoid_: Post, article, berita (in code)

**Announcement**:
A notice with a start and end date, shown on the home page only while it is in effect.
_Avoid_: Notice, pengumuman (in code)

**Event**:
A scheduled village activity with a start time, location and Category, which may be cancelled while staying listed.
_Avoid_: Activity, agenda (in code)

**Page**:
Undated free-form content placed under one of the public site's fixed parent menus.
_Avoid_: Static page, halaman (in code)

**Album**:
An ordered set of photos from one village occasion.
_Avoid_: Gallery (for a single album), galeri (in code)

**Important Contact**:
A phone number residents may need, grouped by contact category.
_Avoid_: Contact, kontak penting (in code)

**Partner**:
An organisation whose logo appears in the home page partner strip.
_Avoid_: Sponsor, mitra (in code)

**Transparency Document**:
An official planning, budget or regulation file published for download, tagged with a document type and fiscal year.
_Avoid_: Attachment, file, dokumen (in code)

**APBDes Summary**:
The public summary of one fiscal year's village budget and, optionally, its realisation.
_Avoid_: Budget, anggaran (in code)

**Category**:
A Village-defined label shared by News Posts and Events.
_Avoid_: Tag, topic, kategori (in code)
