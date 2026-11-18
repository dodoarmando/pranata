# Personal data, archive retention and Komdigi rules for a Village system

Research note for [#23](https://github.com/dodoarmando/pranata/issues/23). As of 2026-11-19.

It builds on two earlier notes and does not repeat them:

- [#4](https://github.com/dodoarmando/pranata/issues/4) (`docs/research/village-regulations.md` on branch `research/village-regulations`) listed the UU 27/2022 duties in outline.
- [#18](https://github.com/dodoarmando/pranata/issues/18) (`docs/research/data-residency.md` on branch `research/data-residency`) covered data residency, the Permenkomdigi 5/2025 storage-location rules, registration in outline, and the three classification levels.

This note answers three things. Has UU 27/2022's implementing PP been enacted, and what does it require of a Village (controller) and the vendor (processor)? How long must village records and logs be kept? What exactly does Permenkomdigi 5/2025 ask for at registration, classification, encryption and export?

Labels used below:

- **Verified**: read in the official text (peraturan.go.id, peraturan.bpk.go.id, or a copy published by the issuing body).
- **Partly verified**: an official source confirms the regulation exists or its status, but the detail comes from a secondary source.
- **Unverified**: secondary sources only. Check before building on it.
- **Interpretation**: our reading of verified texts. No official guidance or ruling confirms it.

## Answer

- **PP 33/2026 exists, but its text has not been verified.** An official DPR news item of 4 November 2026 names _Peraturan Pemerintah Nomor 33 Tahun 2026 tentang Peraturan Pelaksanaan UU PDP_. The text is still not on peraturan.bpk.go.id or peraturan.go.id. Every article number below comes from law-firm and press summaries. They agree on the dates: enacted and promulgated 16 July 2026, in force 16 January 2027 (Art. 225).
- **The statutory duties are already binding.** UU 27/2022 was promulgated on 17 October 2022, and its 2-year adjustment period (Art. 74) ended in October 2024. The duties are:
  - a DPIA for high-risk processing;
  - a DPO for public-service processing;
  - a record of all processing;
  - 3×24-hour deadlines for data-subject requests and breach notices;
  - written approval before a processor engages sub-processors.

  PP 33/2026 adds procedure and evidence on top. No source reports that it names multi-factor authentication or any other specific technical control.
- **No national retention schedule covers village records.** The Archives Law makes destruction lawful only under a _Jadwal Retensi Arsip_ (JRA, archive retention schedule). Destroying records outside that procedure carries up to 10 years' prison. The ministry-level schedules found do not cover a village's own records. In practice the regency sets them by Perbup (regent regulation), and one example applies its regency schedule _mutatis mutandis_ to villages. In that example:
  - population registration and civil registration records are **permanent** (2 years active, 3 inactive, then handed to the regency archive);
  - the letter registers (Agenda, Ekspedisi) are kept 2 + 3 years, then destroyed.

  No rule sets a period for audit or access logs. The only fallback is Permenkominfo 20/2016: stored personal data is kept at least 5 years unless a sector rule says otherwise.
- **Permenkomdigi 5/2025 is stricter than #18 recorded on three points:**
  - **Registrar.** Registration must be filed by a registrar who is a civil servant in the regency's communications and informatics office. A village cannot register itself.
  - **No defaults.** The regulation names three classification levels but never says which data goes where. The regent decides by decree.
  - **Downloads.** Restricted data may be downloaded only with the data steward's approval, and only onto encrypted media (Art. 93). For a spreadsheet export this rule bites before Art. 92 does.

| Question                                             | Finding                                                                                                                                                                                                                                                                                                                 | Status                                                                     |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Is UU 27/2022's implementing PP enacted?             | Yes: PP 33/2026, promulgated 16 July 2026 (LN 2026 No. 88, TLN 7190), in force 16 January 2027                                                                                                                                                                                                                          | Partly verified (DPR names it; text not located on BPK or peraturan.go.id) |
| DPIA                                                 | UU Art. 34: mandatory for high-risk processing, which includes specific data (children's data) and large-scale processing. PP Art. 120–122 reportedly require it before processing starts, with set contents                                                                                                            | UU verified. PP unverified                                                 |
| DPO                                                  | UU Art. 53(1)(a): mandatory when processing serves public services. PP Art. 142–146 add qualifications and independence                                                                                                                                                                                                 | UU verified. PP unverified                                                 |
| Processing records                                   | UU Art. 31: record all processing. PP Art. 74 reportedly lists the contents, including the retention period                                                                                                                                                                                                             | UU verified. PP unverified                                                 |
| Breach notice                                        | UU Art. 46: written, within 3×24 h, to data subjects and the supervisory agency, with minimum contents. PP Art. 114–118 add a public notice when public services are disrupted, and processor-to-controller reporting                                                                                                   | UU verified. PP unverified                                                 |
| Data-subject request deadlines                       | UU: 3×24 h for correction (Art. 30), access (Art. 32), stopping on consent withdrawal (Art. 40), and restriction (Art. 41). No deadline for deletion (Art. 43)                                                                                                                                                          | Verified                                                                   |
| Processing agreement                                 | UU Art. 51: the processor acts on the controller's instructions, and sub-processors need prior written approval. PP Art. 14 reportedly sets the minimum contract terms                                                                                                                                                  | UU verified. PP unverified                                                 |
| Named security measures                              | PP: none named in any summary. Elsewhere: an audit trail (PP 71/2019 Art. 22), encryption of stored personal data (Permenkominfo 20/2016 Art. 15(2)), and encryption at rest of restricted data (Permenkomdigi 5/2025 Art. 90). Two-factor authentication appears only in an elucidation for Electronic Agent operators | Verified (PP content unverified)                                           |
| Retention: population books and supporting documents | No national schedule for village records. Example regency schedule: permanent. Supporting documents for Dukcapil services move to the regency Dukcapil office under its schedule                                                                                                                                        | Verified (example only, not a national rule)                               |
| Retention: letters and Outgoing Letter Agenda        | Example regency schedule and the Ministry of Home Affairs schedule: registers and Agenda 2 + 3 years, then destroy. A letter itself follows its subject                                                                                                                                                                 | Verified (by analogy)                                                      |
| Retention: audit and access logs                     | No period set anywhere found. Fallback: at least 5 years for stored personal data (Permenkominfo 20/2016 Art. 15(3)(b))                                                                                                                                                                                                 | Verified text. Applying it to logs is interpretation                       |
| Komdigi registration contents                        | Art. 9, 11(2), 12: registrar identity; 11 system facts including IP or DNS, risk category, classified data, personal data, storage location; compliance statements                                                                                                                                                      | Verified                                                                   |
| Classification levels and contents                   | Three levels (Art. 64). The regulation does not assign data types to them. The regent's decree does (Art. 83(3))                                                                                                                                                                                                        | Verified                                                                   |
| Encryption at rest                                   | Restricted and closed data (Art. 90). All stored personal data (Permenkominfo 20/2016 Art. 15(2))                                                                                                                                                                                                                       | Verified                                                                   |
| Art. 92 and spreadsheet exports                      | Art. 92 covers off-network delivery. A staff download is access under Art. 93(3): steward approval, encrypted media only                                                                                                                                                                                                | Verified text. Application is interpretation                               |

---

## 1. Is PP 33/2026 real?

**What was checked (2026-11-19):**

- **peraturan.bpk.go.id.** The PP list for 2026 runs only to PP 31/2026. A keyword search for _data pribadi_ (PP type) finds no 2026 PP.
- **peraturan.go.id.** It lists PP 31/2026 (<https://peraturan.go.id/id/pp-no-31-tahun-2026>). <https://peraturan.go.id/id/pp-no-33-tahun-2026> returns 404, and so do PP 32/2026 and PP 34/2026.
- **jdih.komdigi.go.id.** The home page "latest" list shows no PP 33/2026.
- **JDIH Setneg.** Its client-rendered site could not be searched by script.

**Official mention (partly verified).** A DPR news item of 4 November 2026 names it: _"Peraturan Pemerintah Nomor 33 Tahun 2026 tentang Peraturan Pelaksanaan UU PDP"_. The same item says it should speed up forming the personal data protection agency, which shows the agency does not yet exist. <https://www.dpr.go.id/kegiatan-dpr/berita/Taufiq-R-Abdullah-Desak-Pemerintah-Segera-Bentuk-Badan-Pelindungan-Data-Pribadi-70409>

**Metadata (unverified, consistent across sources):**

- enacted and promulgated in Jakarta on 16 July 2026;
- LN 2026 No. 88, TLN 7190;
- 225 articles;
- in force 16 January 2027 (Art. 225);
- Art. 223: processing may continue where it does not conflict with the PP.

Sources:

- RSM Indonesia client alert, 18 September 2026: <https://www.rsm.global/indonesia/sites/default/files/media/publications/Client%20Alert/2026/RSM%20Indonesia%20Client%20Alert%20-%20PP%20332026%20Six%20Implementation%20Priorities%20before%2016%20January%202027.pdf>
- Meridian Hukum metadata page: <https://meridianhukum.com/peraturan/pp-no-33-tahun-2026>
- SIP Law Firm summary: <https://siplawfirm.id/resources/pp-33-2026-pelindungan-data-pribadi>
- Veritask article: <https://veritask.ai/en/artikel/technical-regulation-on-personal-data-protection-and-the-obligations-of-controllers-and-processors-finally-issued-through-government-regulation-no-33-of-2026>
- Katadata, 3 September 2026: <https://katadata.co.id/finansial/makro/6a9979b7c5745/pp-pelindungan-data-pribadi-berlaku-januari-2027-ini-aturan-baru-data-anak>

**Supervisory agency.** UU 27/2022 sends breach notices and complaints to the _lembaga_ (agency). Press and DPR sources from September to November 2026 say it has not been formed. Until it exists, a breach notice to "the agency" has no recipient, but the notice to data subjects still applies. Partly verified (DPR item).

## 2. Duties of the Village (controller) and the vendor (processor)

UU 27/2022 source (BPK, verified): <https://peraturan.bpk.go.id/Download/224884/UU%20Nomor%2027%20Tahun%202022.pdf>. The "PP 33/2026" column is **unverified**, compiled from the sources in section 1.

### DPIA

- **UU Art. 34 (verified).** A DPIA is mandatory when processing has potential high risk, including:
  - automated decisions with legal or significant effect;
  - processing of specific data;
  - large-scale processing;
  - systematic evaluation, scoring or monitoring;
  - matching or combining data sets;
  - new technology;
  - processing that limits data-subject rights.
- **Why it applies.** Children's data is specific data (Art. 4(2)), so a village population register always triggers a DPIA (see #4).
- **PP 33/2026 (unverified):**
  - Art. 120(2) repeats the seven triggers. The Elucidation names AI, machine learning, smart technology and IoT as "new technology".
  - Art. 121(1): the DPIA is done before processing begins. Art. 121(5): it is reviewed when the risk changes.
  - Contents: a description and the purposes, necessity and proportionality, risks to data subjects, and mitigation. The DPO's advice is documented.
  - Art. 122: in some cases the controller consults the agency.
  - Detailed methodology is left to agency regulation.

### DPO (_pejabat atau petugas pelindungan data pribadi_)

- **UU Art. 53(1) (verified).** Controllers and processors must appoint one when:
  - (a) processing is for public services;
  - (b) core activities need regular, systematic, large-scale monitoring; or
  - (c) core activities are large-scale processing of specific data.
- **UU Art. 53(2)–(3).** The appointment is based on professionalism, legal knowledge, data-protection practice and ability. The DPO may come from inside or outside the organisation.
- **Constitutional Court.** Decision 151/PUU-XXII/2024 reportedly reads the conditions as "and/or", so any one suffices. Unverified.
- **Interpretation (high confidence).** Village administrative services are public services, so every Village running Pranata must appoint a DPO. Art. 53(1) also names the processor, so the vendor needs one too.
- **PP 33/2026 (unverified).**
  - Art. 142: same three triggers.
  - Art. 143: qualifications.
  - Art. 144–146: tasks (advice, compliance monitoring, DPIA input, contact point), access to top management, independence, no conflict of interest, resources.

### Record of processing

- **UU Art. 31 (verified).** The controller must record all processing activities. UU Art. 52 extends this to the processor.
- **UU Art. 32 (verified).** Access requests return the data together with its processing history (_rekam jejak pemrosesan_). The record therefore has to be traceable per data subject.
- **PP 33/2026 Art. 74 (unverified).** Contents:
  - identities of the controller, processor and DPO;
  - data sources;
  - legal basis and purposes;
  - data categories;
  - third-party recipients;
  - retention period;
  - safeguards;
  - transfer details.
- **Accountability (unverified).** Art. 138 requires internal and external data-protection audits of controllers. Art. 139–141 require the same of processors. No frequency is set.

### Breach notice

- **UU Art. 46 (verified):**
  - **Deadline and recipients.** A written notice within 3×24 hours to (a) the data subjects and (b) the agency.
  - **Minimum contents.** The data revealed; when and how it was revealed; and the handling and recovery steps.
  - **Public notice.** "In certain cases" the public must be told.
- **PP 33/2026 (unverified):**
  - **Clock start.** Art. 114: the clock starts when the failure is known "with certainty, reasonably and properly" (_pasti, patut, dan wajar_), on evidence such as system logs and forensic reports.
  - **Contents.** The notice adds the DPO or contact person.
  - **Public notice.** Art. 115 requires one when the breach **disrupts public services** or seriously affects the public interest. This plausibly covers a Village outage caused by a breach.
  - **Documentation.** Art. 116 covers the data affected, the chronology, the impact, recovery and the notice times.
  - **Internal policy.** Art. 117 requires an incident policy covering prevention, roles, escalation and post-incident review.
  - **Processor reporting.** Art. 118: the processor reports to the controller "at the first opportunity".
- **Older rule.** Permenkominfo 20/2016 Art. 28(c) set 14 days. _Interpretation_: UU Art. 46 supersedes it, because UU Art. 75 keeps older rules only where they do not conflict.

### Data-subject requests (verified, UU 27/2022)

| Right                                                      | Article    | Deadline                                                 |
| ---------------------------------------------------------- | ---------- | -------------------------------------------------------- |
| Update or correct                                          | Art. 30(1) | 3×24 h from receipt, then notify the result (Art. 30(2)) |
| Access, with processing history                            | Art. 32(2) | 3×24 h from receipt                                      |
| Stop processing after consent withdrawal                   | Art. 40(2) | 3×24 h from receipt                                      |
| Suspend or restrict                                        | Art. 41(1) | 3×24 h from receipt (exceptions in Art. 41(2))           |
| End processing (retention reached, purpose met, request)   | Art. 42    | None stated                                              |
| Delete                                                     | Art. 43    | None stated                                              |
| Destroy (retention ended under the JRA, request, unlawful) | Art. 44    | None stated                                              |

- **Exceptions (Art. 50(1)).** Art. 30, 32, 42, 43(1)(a)–(c), 44(1)(b), 45 and 46(1)(a) do not apply where processing serves defence, law enforcement, "the public interest in running the state", or financial-sector supervision. _Interpretation_: a Village should not rely on the "running the state" exception for routine services without guidance.
- **PP 33/2026 (unverified).**
  - Art. 20–27: the controller provides an accessible channel, records each request, and verifies identity proportionately.
  - If processing is electronic, the request channel must be electronic.
  - For copy requests, the 3×24 h clock starts after verification.
  - Art. 27: unanswered requests can be reported to the agency.

### Processor agreement and sub-processors

- **UU Art. 51 (verified).**
  - The processor acts on the controller's instructions (para. 1), and that processing is the controller's responsibility (para. 3).
  - Sub-processors need the controller's prior written approval (para. 5).
  - Processing outside instructions becomes the processor's own responsibility (para. 6).
- **UU Art. 52 (verified).** Art. 29, 31 and 35–39 also bind the processor: accuracy, processing records, security, confidentiality, supervision, protection from unlawful processing and access.
- **PP 33/2026 (unverified).**
  - Art. 13–16: a written agreement with minimum terms. These are the scope and means of processing, the types and purposes, the categories of data subjects, the period, each party's rights and obligations, and supervision and audit.
  - Art. 15: a sub-processor needs prior written approval and an equivalent level of protection.
  - Clauses that reduce or remove the controller's obligations are reportedly prohibited.

### Children's data

- **UU Art. 25 (verified).** Children's data is processed "specifically" and requires parental or guardian consent.
- **PP 33/2026 Art. 38 (unverified).**
  - The controller identifies child users before processing.
  - It obtains parental consent and verifies it with the technology available. A tick box is reportedly not enough.
  - Art. 38(5) covers how a child's own rights requests are handled.
- **Interpretation.** A village register of children rests on a legal duty (Adminduk and Permendagri 47/2016), not on consent (UU Art. 20(2)(c) legal obligation, or (e) public service and official authority). Whether Art. 25(2) still demands parental consent there is unclear; the text says _wajib_ without qualification. Open question 3.

### Named security measures

No summary of PP 33/2026 reports a named control such as multi-factor authentication, encryption or logging. Unverified, since the text was not read.

The verified texts that do name measures:

| Measure                                                                                                                                                                                                 | Source                                  | Who it binds                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ----------------------------------------------------------------- |
| Technical and operational measures, and a security level set by the nature and risk of the data                                                                                                         | UU 27/2022 Art. 35                      | Controller and processor (Art. 52)                                |
| Prevent unauthorised access with a reliable security system                                                                                                                                             | UU 27/2022 Art. 39                      | Controller and processor                                          |
| **Audit trail** of all system activity. The Elucidation says keep transaction logs "per the operator's data retention policy" and have audit-trail functions to detect intrusion, reviewed periodically | PP 71/2019 Art. 22 and Elucidation      | Every electronic system operator                                  |
| Security procedures and a prevention system. The Elucidation lists antivirus, anti-spam, firewall, intrusion detection and prevention, and an information-security management system                    | PP 71/2019 Art. 24 and Elucidation      | Every electronic system operator                                  |
| A security system, personal-data protection, and a system feasibility test (_uji kelaikan_)                                                                                                             | Permenkomdigi 5/2025 Art. 4(3)          | Public-scope operators                                            |
| **Stored personal data must be encrypted**                                                                                                                                                              | Permenkominfo 20/2016 Art. 15(2)        | Every electronic system operator                                  |
| Audit trail                                                                                                                                                                                             | Permenkominfo 20/2016 Art. 28(e)        | Every electronic system operator                                  |
| Restricted and closed data encrypted at rest; restricted data encrypted in transit                                                                                                                      | Permenkomdigi 5/2025 Art. 90, 91(2)(c)  | Public-scope operators                                            |
| **Two-factor authentication.** "A combination of at least 2 authentication factors" when testing user identity                                                                                          | Elucidation of PP 71/2019 Art. 40(1)(a) | Electronic Agent operators only (_Penyelenggara Agen Elektronik_) |

- **Sources.**
  - PP 71/2019 (BPK): <https://peraturan.bpk.go.id/Download/112816/PP%20Nomor%2071%20Tahun%202019.pdf>
  - Permenkominfo 20/2016: BPK lists it as in force (_Berlaku_) with no amendment recorded. <https://peraturan.bpk.go.id/Details/150543/permenkominfo-no-20-tahun-2016>, text <https://peraturan.bpk.go.id/Download/142743/Permen%20Kominfo%20Nomor%2020%20Tahun%202016.pdf>
- **Interpretation.** No verified rule makes multi-factor authentication mandatory for a village records system. The PP 71/2019 elucidation is the clearest official signal that it is expected for authenticating users of a sensitive system. Unverified: whether PP 33/2026 revokes Permenkominfo 20/2016.

## 3. Retention periods

### The legal frame (verified)

- **UU 43/2009 (Archives Law)**, in force. BPK: <https://peraturan.bpk.go.id/Details/38788/uu-no-43-tahun-2009>
  - **Duty to have a JRA.** Art. 48: state institutions, regional governments, state universities and SOEs must have a JRA, set by their head. Villages are not named.
  - **Destruction.** Art. 51: records may be destroyed only if:
    - they have no further use;
    - their retention has ended and the JRA marks them "destroy";
    - no law forbids it; and
    - they are not tied to an unresolved case.
  - **Penalty.** Art. 86: deliberately destroying records outside the proper procedure carries up to 10 years in prison and a fine of up to Rp500 million.
  - **Villages.** Art. 24(4)(b): the regency archive manages the static (permanent) records it receives from villages.
- **UU 27/2022 Art. 44(1)(a).** The controller must destroy personal data whose retention has ended _and that the JRA marks for destruction_. Personal-data retention is therefore tied to the archive schedule.
- **Permenkomdigi 5/2025 Art. 64(5).** Classified data _is_ archives under archive law.
  - Art. 94: the data-owning _Instansi_ sets the retention period under the law.
  - Art. 95: destruction must be secure, including copies held by third-party cloud providers. It requires expired retention, no further use, legal compliance, and no pending case.
- **Permenkominfo 20/2016 Art. 15(3).** Personal data in an electronic system is kept for the period a sector rule sets, or **at least 5 years** if no rule exists. Art. 16 counts that period from when the person stopped being a user.

### What does not cover village records (verified)

- **Permendagri 47/2016** (village administration books). Its body sets no retention period and does not mention archives, storage or electronic records. Source: <https://peraturan.go.id/files/bn1100-2016.pdf>
- **Permendagri 20/2024** (JRA and security classification). It covers the Ministry of Home Affairs' **own** records (Art. 2(1), Art. 1 point 14).
  - Its "village government" and "population and civil registration" rows cover ministry policy files, not a village's books.
  - It repealed Art. 9 and Lampiran D of Permendagri 78/2012 (Art. 12).
  - BPK: <https://peraturan.bpk.go.id/Details/321578/permendagri-no-20-tahun-2024>
- **Perka ANRI 20/2015** (retention guideline, _population and family planning_). This is the family-planning agency's (BKKBN) functions: population control, contraception programmes. It does not cover population registers. BPK: <https://peraturan.bpk.go.id/Details/276951/perka-arsip-nasional-no-20-tahun-2015>
- **Permendagri 104/2019** (_Pendokumentasian Administrasi Kependudukan_, documentation of population administration). It covers documents of the regency civil registration office (_Disdukcapil_). These include completed registration forms, copies of supporting documents, and books used in registration, including those handled at village level (Art. 2(2), 3(3)).
  - Inactive documents held at the village move to the regency Disdukcapil under its retention schedule (Art. 9(4)(a)).
  - Periods are left to other rules (Art. 10(1)).
  - BPK: <https://peraturan.bpk.go.id/Details/138592/permendagri-no-104-tahun-2019>

No ANRI retention guideline for village government affairs was found on BPK. A 2014 ANRI article reportedly listed one among its guidelines (unverified).

### How it works in practice: the regency schedule (verified example)

Regencies set JRAs by Perbup, and some extend them to villages. The example checked is **Perbup Kabupaten Banyumas No. 22 of 2025** _tentang Jadwal Retensi Arsip di Lingkungan Pemerintah Daerah dan Pemerintah Desa_. It was set on 23 May 2025 and is listed as in force. BPK: <https://peraturan.bpk.go.id/Details/327530>, text <https://peraturan.bpk.go.id/Download/389304/Peraturan-Nomor-22-Tahun-2025.pdf>

- **Mutatis mutandis.** Art. 8: the regency schedule applies _mutatis mutandis_ to the village government. The village follows the regency rows by subject.
- **Counting.** Art. 5: active retention runs from creation and registration until the matter is finished. Inactive retention runs from the end of the active period.

| Record                                                                                                 | Row in the regency schedule | Active  | Inactive | Then                                   |
| ------------------------------------------------------------------------------------------------------ | --------------------------- | ------- | -------- | -------------------------------------- |
| Population registration: identity, moving in and out within Indonesia and abroad, vulnerable residents | L.2(a)–(d)                  | 2 years | 3 years  | **Permanent** (to the regency archive) |
| Civil registration: births and deaths, marriages and divorces, adoption, citizenship                   | L.3(a)–(d)                  | 2 years | 3 years  | **Permanent**                          |
| Population information management: data management, the SIAK system                                    | L.4(a)–(c)                  | 2 years | 3 years  | Permanent                              |
| Population information services                                                                        | L.4(d)                      | 2 years | 3 years  | Destroy                                |
| Incoming and outgoing letter registers, **Agenda**, control cards, **Ekspedisi**                       | A.3(a) (records management) | 2 years | 3 years  | Destroy                                |

The Ministry of Home Affairs' own schedule has the same row for registers, Agenda and Ekspedisi: 2 + 3 years, destroy (Permendagri 20/2024, Lampiran, Fasilitatif E.2(a)).

- **Interpretation.**
  - **Population books.** The village population books (Permendagri 47/2016 B.1–B.5) are population-registration records, so under a schedule like this they are **permanent**. The resident register and the mutation register must never be purged. They eventually go to the regency archive, not to deletion.
  - **Supporting documents.** Copies of documents for Dukcapil services follow the Disdukcapil schedule (Permendagri 104/2019).
  - **Issued letters.** Each letter takes the row of its subject and classification code (Permendagri 83/2022). Most village _surat keterangan_ (certificates) fall under population or public-service rows.
  - **The Outgoing Letter Agenda** is a register: 5 years in total, then destroy.
  - Other regencies may differ. Each Installation needs its own regency's Perbup.

### Audit and access logs

- No regulation found sets a period for audit or access logs.
- PP 71/2019 Elucidation of Art. 22 defers to "the operator's data retention policy, in line with the law".
- The Ministry of Home Affairs schedule's IT rows cover records _about_ IT management, not system logs. Example: "data security management", 1 + 4 years, destroy.
- **Interpretation.** Access logs hold personal data (who viewed which resident). Under Permenkominfo 20/2016 Art. 15(3)(b), keep them **at least 5 years**. They are also the evidence for:
  - access requests with processing history (UU Art. 32);
  - breach timelines (PP 33/2026 Art. 114, unverified);
  - audits (Art. 138, unverified).

  A 5-year default for audit and access logs is defensible. Until a regency schedule says otherwise, the Village should confirm it in writing.

## 4. Permenkomdigi 5/2025: the details #18 left open

Source (BPK, verified): <https://peraturan.bpk.go.id/Download/378675/Permenkomdigi%205%20tahun%202025.pdf>

### Who registers, and what is submitted

- **Registrar (Art. 5–6).** Every public-scope operator appoints a registrar. For an _Instansi_ the registrar must be a civil servant at least at _administrator_ level, or a senior functional IT expert (_ahli madya_). The registrar sits **in the work unit or regional office responsible for communications and informatics**, and is assigned by letter.
  - _Interpretation_: a Village has no such official. Its systems would be registered by the regency's communications and informatics office (Diskominfo) registrar, on the Village's behalf.
- **Registrar details (Art. 9(2)).** The registrar uploads the assignment letter and gives:
  - the type and name of the institution;
  - the work unit and its phone number;
  - their status, full name and employee number;
  - their job title and mobile number;
  - the official institutional email.
- **System form (Art. 11(2)).** Statements of compliance with:
  - (b) information-security duties;
  - (c) the security system, including prevention and response;
  - (d) personal-data protection;
  - (e) the system feasibility test;
  - (f) the national and regional e-government (SPBE) architecture.
- **General description of the system (Art. 12):**
  - (a) system name;
  - (b) owner;
  - (c) sector;
  - (d) contact person;
  - (e) website URL;
  - (f) server DNS name and/or IP address;
  - (g) a short description of functions and business processes;
  - (h) risk category;
  - (i) classified data held;
  - (j) personal data processed;
  - (k) where the system and data are managed, processed and stored.
- **After approval (Art. 13).**
  - Komdigi publishes a registration certificate in its public list. The certificate includes the system's name, function and **version**.
  - The operator must display the certificate on the registered system (Art. 13(5)).
  - The certificate is a precondition for IT spending clearance (Art. 13(6)).
- **Updates (Art. 14–16).**
  - Any change to the form data must be re-filed (Art. 14).
  - Retiring a system needs a letter (Art. 15).
  - Sanctions (Art. 16): blocking if unregistered. Wrong or stale data draws three warnings, each a week apart, then suspension, then blocking and removal from the list.

### Classification: levels and who fills them

- **Duty and steps (Art. 62).** Every public-scope operator classifies its data by risk in three steps: risk impact areas, impact criteria, risk level.
- **Who decides (Art. 63).** The operator uses the classified-data list set by the head of the _Instansi_ or the **regional head**, under One Data Indonesia rules. Where no list exists, the One Data Indonesia forum discusses it.
- **Levels (Art. 64):**
  - _terbuka_ (open): low risk;
  - _terbatas_ (restricted): medium risk;
  - _tertutup_ (closed): high risk.

  Classified data counts as archives (Art. 64(5)).
- **No defaults.** The regulation gives **no examples** and does not assign personal or population data to any level.
- **Regional process (Art. 73–83).**
  - The regent sets the list **by decree** (Art. 83(3)), on a proposal from the regional data steward (_Walidata_).
  - The regent follows a regional classification guideline from the Ministry of Home Affairs (Art. 83(2)). No such guideline was found as of 2026-11-19.
  - The _Walidata_ approves access to restricted data (Art. 77(1)(j)). The regent approves access to closed data (Art. 74(b)).
- **Review (Art. 84).** At least once every 3 years for open data, 5 for restricted, and 15 for closed.
- **What lands where (interpretation).** For closed data, access is limited to officials at least at senior executive level (_pimpinan tinggi pratama_), in a special secured room, read-only, on registered devices (Art. 93(6)). A village official could never meet that.
  - If a regency classed village resident data as _tertutup_, a village system could not lawfully be used by village staff. Village population data must therefore sit at _terbatas_ for the system to work.
  - Public site content and aggregate statistics are _terbuka_.

### Encryption at rest

- **Art. 90 (verified).** _"Data Elektronik terbatas dan tertutup disimpan (at-rest) dalam keadaan terenkripsi."_
- **Closed data held at the _Instansi_** must sit in encrypted storage in its own computing centre (Art. 89).
- **Permenkominfo 20/2016 Art. 15(2)** independently requires all stored personal data to be encrypted.
- **Interpretation.** Encrypt the database volume, file storage and backups. Pranata cannot rely on the Village's classification decree arriving first.

### Moving data out, and spreadsheet exports

- **In-network delivery (Art. 91).** Restricted data sent over a network needs integrity and authenticity, goes through the **government intranet**, and is encrypted in transit. Closed data may not be shared over a network.
- **Off-network delivery (Art. 92).**
  - Allowed only when in-network delivery has obstacles or the file is too large (para. 1).
  - Restricted data needs **encryption with an unpublished algorithm supplied by the cyber-security agency (BSSN)**, in **sealed media labelled "Terbatas"** provided by the receiving _Instansi_ (para. 3).
  - Closed data may not go off-network on any media (para. 4).
- **Access (Art. 93(3)).** Access to restricted data needs **the _Walidata_'s approval**, and the data "may be downloaded and stored on **encrypted media**" only. The data-owning _Instansi_ sets the approval requirements (para. 5).
- **How this applies to a spreadsheet export of resident data (interpretation):**
  1. **A staff member downloading an export** is _access_ (Art. 93). It needs an approval rule and must land on encrypted media. An unencrypted .xlsx on a shared office PC does not comply.
  2. **Emailing that export** to the regency is in-network delivery (Art. 91(2)). On a strict reading it should go over a government network, encrypted. Public email does not comply.
  3. **Handing it over on a flash drive** is off-network delivery (Art. 92(3)). It needs BSSN encryption, which a village cannot do on its own.
  4. The **monthly recap** (B.3) is aggregate counts per _dusun_. If classified _terbuka_, it may move freely.

## Implications for Pranata

- **DPIA and DPO kit.** Ship a DPIA template pre-filled for Pranata's processing: specific data (children), a population register, and a public website. Ship a DPO appointment template (a village decree, _SK Kepala Desa_). The vendor appoints its own DPO as processor (UU Art. 53).
- **Processing record from the audit log.** Generate the UU Art. 31 processing record from configuration: purposes, legal basis, data categories, recipients, retention, safeguards. Keep a per-resident processing history, so an access request (Art. 32) can be answered within 3×24 h with one export.
- **Request handling.** Add a request log with a 72-hour timer for correction, access, consent withdrawal and restriction. Add an identity-verification step before the clock runs (PP, unverified). Add an electronic request channel (PP Art. 20–27, unverified).
- **Breach playbook.**
  - A 72-hour timer from confirmation.
  - Notice templates for data subjects and the agency (record "agency not yet formed" until it exists), and a public notice when services are disrupted.
  - Processor-to-Village reporting "at the first opportunity" written into the contract.
- **Processing agreement.** Write a vendor–Village agreement with the reported PP Art. 14 terms: scope and means, types and purposes, subject categories, period, rights and obligations, supervision and audit. Name the sub-processors with written approval (Art. 51(5)) and add no exoneration clause. Merge in the #18 residency terms.
- **Retention as data, not code.** Each record type carries a retention rule (active years, inactive years, permanent or destroy), loaded from the Installation's regency schedule. Defaults from the example:
  - resident and mutation registers: permanent, never purged; archive export, no hard delete;
  - Agenda and Ekspedisi: 2 + 3 years, then destroy;
  - issued letters: by classification code;
  - audit and access logs: 5 years minimum.

  Destruction is a recorded, approved act (UU 43/2009 Art. 51, 86) with a minutes record (_berita acara_), never an automatic cron delete.
- **Separate erasure from archiving.** UU 27/2022 deletion requests (Art. 43) cannot remove statutory register entries that are permanent archives. Handle such requests by restricting and correcting, and record why.
- **MFA by default.** No verified rule mandates it, but the PP 71/2019 elucidation points that way, and the data is restricted. Require two-factor login for staff accounts that can read resident data.
- **Encryption.** Encrypt at rest everywhere: volume, files and backups (Art. 90, Permenkominfo 20/2016 Art. 15(2)). Use TLS in transit.
- **Komdigi registration pack.** Per Installation, produce a filled sheet of the Art. 12 fields (a)–(k) plus the Art. 11(2) compliance statements. The regency Diskominfo registrar files it. Show the registration certificate in the admin footer (Art. 13(5)). Re-file when the IP, server, provider or major version changes.
- **Default classification.**
  - Public site content and aggregates: _terbuka_.
  - Resident, family and letter records: _terbatas_.
  - Nothing _tertutup_, since village staff could not access it.

  The Village asks its regency to confirm by decree.
- **Exports.**
  - Gate resident-level exports behind a named approver role (the _Walidata_ analogue).
  - Produce encrypted files (for example password-protected archives) instead of plain .xlsx, and log every export.
  - Prefer in-system sharing, or the regency's government channel, over email.
  - Keep aggregate reports (B.3) free of personal data so they can move as _terbuka_.

## Open questions and unverified items

1. **PP 33/2026 official text.** Confirm the articles cited above (14–15, 20–27, 38, 74, 114–118, 120–122, 138–146, 223, 225) on peraturan.go.id or JDIH Setneg once published. Check in particular whether it:
   - names any technical control (MFA, encryption, logging);
   - sets request deadlines beyond the UU;
   - repeals Permenkominfo 20/2016.
2. **The supervisory agency.** When it is formed, add it as the breach-notice recipient and check its DPIA methodology and standard contract clauses.
3. **Children's data in a statutory register.** Does UU Art. 25(2) (parental consent) apply when the legal basis is a legal obligation? Check PP 33/2026 Art. 38 and any agency guidance.
4. **The Installation's own regency JRA.** Each pilot regency's Perbup must be read. Only one example was checked, and others may set different periods or not cover villages at all.
5. **ANRI guideline for village government affairs.** A 2014 ANRI article reportedly lists one; it was not found on BPK. Search jdih.anri.go.id.
6. **Ministry of Home Affairs regional data classification guideline** (Permenkomdigi 5/2025 Art. 83(2)). Not found. Also check the pilot regency's classification decree, if any.
7. **Art. 91 and staff access over the internet.** Does staff using a web app over the public internet count as in-network delivery that must use the government intranet? A strict reading of Art. 91(2)(b) would conflict with any internet-hosted village system. Ask Komdigi or the regency Diskominfo.
8. **The Constitutional Court ruling on DPO triggers** (151/PUU-XXII/2024). Read the decision on mkri.id.
9. **Log retention.** No source sets a period for audit or access logs. Confirm the 5-year default with the regency archive office.
