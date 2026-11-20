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

### Village

**Village Code**:
The Village's 10-digit government region code, which fixes its name, its district, regency and province, and its time zone.
_Avoid_: Kode desa (in code), village ID

**Vision and Mission**:
The Village's current vision statement and ordered missions, together with the RPJMDes period they belong to.
_Avoid_: Visi misi (in code), goals

**Region Unit**:
Any node of the Village's internal region tree: a Hamlet, an RW, or an RT. It is deactivated, never deleted, once in use.
_Avoid_: Area, zone, wilayah (in code)

**Hamlet**:
The top-level Region Unit below the Village, called dusun by default and by a Village-chosen name elsewhere (banjar, jorong, lingkungan); the unit the monthly population recap is grouped by.
_Avoid_: Dusun (in code), district, sub-village

**RW**:
The optional middle Region Unit, belonging to one Hamlet and numbered uniquely across the Village.
_Avoid_: Rukun warga (in code), block

**RT**:
The lowest Region Unit, belonging to one RW (or to one Hamlet when the Village has no RW level) and numbered uniquely within it; every Family address points to one.
_Avoid_: Rukun tetangga (in code), neighbourhood

**Region Correction**:
Moving Families from one RT to another because the region tree changed, not because anyone moved; it is not a population event.
_Avoid_: Transfer, mutation, move

**Position**:
A post in the Village government that Officials hold, such as head of village, secretary, or the head of one Hamlet.
_Avoid_: Jabatan (in code), role, title

**Tenure**:
One period during which an Official holds a Position, either permanently (definitif), as a temporary appointee (Pj.), or as an acting holder (Plt.); never deleted.
_Avoid_: Term, assignment, masa jabatan (in code)

**Official**:
A person holding or having held a Tenure in the Village government (perangkat desa). An Official is not a Resident record and need not be a User.
_Avoid_: Staff, officer, perangkat (in code)

**Institution**:
A body outside the Village government (BPD, LPM, PKK, BUMDes and the like) with an ordered list of members, none of whom sign letters.
_Avoid_: Organisation, lembaga (in code)

**Indeks Desa Edition**:
The ministry's score and status for the Village for one data-collection year, under either the Indeks Desa or the older IDM.
_Avoid_: Ranking, IDM score (for current editions)

### Population

**Resident**:
A person registered as living in the Village, identified by an NIK when one is known; never deleted, only made inactive by a Death or Departure.
_Avoid_: Citizen, person, warga, penduduk (in code)

**NIK**:
A person's 16-digit national identity number, unique across every Resident record the Installation has ever held.
_Avoid_: ID number, KTP number

**Family**:
One family card (Kartu Keluarga): its number (which may be not yet known), its head, and its address at one RT; every active Resident belongs to exactly one, and it is active while it has an active member.
_Avoid_: Household, KK (in code), keluarga (in code)

**Population Event**:
A dated Birth, Death, Arrival, or Departure of one Resident: the only thing that changes the Village's population count. It is voided with a reason, never edited or deleted.
_Avoid_: Mutation, change, peristiwa (in code)

**Birth**:
The Population Event that adds a newborn to a Family of the Village, wherever the birth took place.
_Avoid_: Kelahiran, lahir (in code)

**Death**:
The Population Event that makes a Resident inactive because they died.
_Avoid_: Kematian, meninggal (in code)

**Arrival**:
The Population Event that adds a person moving into the Village, reusing their earlier Resident record if they once lived here.
_Avoid_: Kedatangan, datang (in code), move in

**Departure**:
The Population Event that makes a Resident inactive because they moved out of the Village.
_Avoid_: Kepindahan, pindah (in code), move out, transfer

**Baseline**:
The population as it stood when the Village locked its initial import, which is the opening balance of the first Monthly Recap; after it, people join only through Births and Arrivals.
_Avoid_: Initial data, go-live, opening import

**Monthly Recap**:
The per-Hamlet count of Residents and Families for one month, with that month's Births, Deaths, Arrivals and Departures (Permendagri 47/2016 book B.3).
_Avoid_: Monthly report, rekap (in code)

**Sent**:
The state of a Monthly Recap that has been reported to the district; it is frozen, and later Population Events dated in that month are counted in the first month not yet Sent.
_Avoid_: Final, locked, submitted, terkirim (in code)

**Resident Register**:
The book of all active Residents, Family by Family (Permendagri 47/2016 book B.1).
_Avoid_: Buku induk (in code), population list

**Mutation Register**:
The month-by-month book of Population Events that are not voided (Permendagri 47/2016 book B.2).
_Avoid_: Buku mutasi (in code), event log

**Temporary Stay**:
One stay in the Village by a person who is not a Resident, from arrival to departure, recorded with their host; voided with a reason, never deleted, and never part of the population count (Permendagri 47/2016 book B.4).
_Avoid_: Temporary Resident, visitor, guest, penduduk sementara (in code)

**Temporary Stay Register**:
The yearly book of Temporary Stays that are not voided, in order of arrival (Permendagri 47/2016 book B.4).
_Avoid_: Buku penduduk sementara (in code), guest book

### Letters

**Letter Type**:
A Village-defined kind of Service Letter: its name, classification code, Layout, additional fields and statement text.
_Avoid_: Template, form, jenis surat (in code)

**Layout**:
A printed letter design that Pranata itself ships, such as Standard or the marriage cover letter (N1), which a Letter Type is built on.
_Avoid_: Template, theme, tata letak (in code)

**Service Letter**:
A letter Pranata renders for one Resident from a Letter Type, signed on paper by one Official.
_Avoid_: Certificate, document, surat layanan (in code)

**Recorded Letter**:
A letter written outside Pranata that is only recorded so it takes a Letter Number from the same sequence.
_Avoid_: Manual letter, external letter, surat tercatat (in code)

**Letter Number**:
The number a Service Letter or Recorded Letter receives from the Village's single yearly sequence when it is issued or recorded; never reused, even when the letter is Cancelled.
_Avoid_: Reference number, nomor surat (in code)

**Issued**:
The state of a Service Letter that has its Letter Number and its frozen content; it is never edited, only Cancelled.
_Avoid_: Published, final, terbit (in code)

**Cancelled**:
The state of a letter withdrawn with a reason; it keeps its Letter Number and stays in the Outgoing Letter Agenda.
_Avoid_: Deleted, voided, revoked, dibatalkan (in code)

**Verification Code**:
The short code printed on an Issued Service Letter, with its QR code, that lets anyone check on the public site that the letter is genuine and not Cancelled.
_Avoid_: Token, serial, kode verifikasi (in code)

**Letter Request**:
A request for a Service Letter of one online-requestable Letter Type, submitted on the public site by a requester (the subject or someone acting for them) and reviewed by staff, who reject it with a reason or turn it into a Draft for the matched Resident; the letter is collected on paper at the village office.
_Avoid_: Application, submission, ticket, permohonan (in code)

**Request Code**:
The random code a requester receives when submitting a Letter Request, the only way to follow its status on the public site.
_Avoid_: Tracking number, ticket number, kode permohonan (in code)

**Outgoing Letter Agenda**:
The book of every Letter Number given in one year, in order, covering both Service Letters and Recorded Letters (the outgoing half of Permendagri 47/2016 book A.7).
_Avoid_: Letter log, buku agenda (in code)

### Complaints

**Complaint**:
A complaint or suggestion a resident sends through the public site, possibly anonymously, that staff answer with Complaint Responses and then close, or reject with a reason; never shown publicly.
_Avoid_: Ticket, report, aspiration, feedback, pengaduan (in code)

**Complaint Topic**:
A Village-defined subject a Complaint is filed under, used to sort and count Complaints.
_Avoid_: Category (reserved for News Posts and Events), topik (in code)

**Complaint Response**:
A message from staff on a Complaint, shown to the complainant on its status page and never edited once posted.
_Avoid_: Reply, comment, follow-up, tanggapan (in code)

**Complaint Code**:
The random code a complainant receives when sending a Complaint, the only way to read its status and Complaint Responses on the public site.
_Avoid_: Ticket number, Request Code (reserved for Letter Requests), kode pengaduan (in code)

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

### Records

**Retention Schedule**:
The Village's keeping period for each kind of record, taken from its regency's archive retention schedule (JRA); population records are kept permanently.
_Avoid_: Retention policy, TTL, jadwal retensi (in code)

**Destruction**:
An approved act, recorded with the number and date of its destruction report (berita acara), that erases the personal data of records past their Retention Schedule while keeping their statistical skeleton; never automatic.
_Avoid_: Deletion, purge, erasure, pemusnahan (in code)

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
A phone number residents may need, grouped by Contact Category.
_Avoid_: Contact, kontak penting (in code)

**Contact Category**:
A Village-defined, ordered label that Important Contacts are grouped under on the public site; one in use is kept.
_Avoid_: Category (reserved for News Posts and Events), kategori kontak (in code)

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

**Quick Link**:
A Village-chosen shortcut to a page of the public site, shown under the home page search box.
_Avoid_: Popular search, tag, tautan cepat (in code)
