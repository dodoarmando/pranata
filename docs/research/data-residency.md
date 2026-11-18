# Data residency for a Village's data

Research note for [#18](https://github.com/dodoarmando/pranata/issues/18). As of 2026-11-19.

[#12](https://github.com/dodoarmando/pranata/issues/12) assumed, from legal press coverage, that PP 71/2019 Art. 20 makes a Village government keep its data in Indonesia, so the VPS and the off-site backups must sit in Indonesian data centres. This note checks that assumption against the official texts. It also checks whether later regulations changed the rule, and whether the rule reaches the vendor that runs an Installation for the Village.

Labels used below:

- **Verified**: read in the official text (peraturan.go.id, peraturan.bpk.go.id, or a copy published by the issuing body).
- **Partly verified**: the official source confirms the regulation exists or its status, but the detail comes from a secondary source.
- **Unverified**: secondary sources only. Check before building on it.
- **Interpretation**: our reading of verified texts. No official guidance or ruling confirms it.

## Answer

**Yes.** A Village's electronic system and data must be managed, processed and stored in Indonesia. The only exception is when the storage technology is not available in Indonesia, as decided by a government committee. Commodity VPS hosting and object storage are available in Indonesia, so the exception does not apply to Pranata.

The rule is stricter than #12 assumed. Since 25 March 2025, Permenkomdigi 5/2025 implements Art. 20 and adds three things:

1. Public-scope data must sit in the **Pusat Data nasional** (national data centre). That means government data-centre or cloud services, or **third-party cloud providers registered with Komdigi**.
2. The Village must **register its system** with Komdigi, and the registration states where the data is stored.
3. The Village must use a **`.desa.id`** domain.

| Question                                                 | Finding                                                                                                                                                                                                   | Status                                                                    |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Does PP 71/2019 Art. 20(2) require storage in Indonesia? | Yes, for public-scope operators. Exact text below                                                                                                                                                         | Verified                                                                  |
| Is a Village government a public-scope operator?         | Yes. It is an _Instansi_, and Permenkomdigi 5/2025 treats it as one                                                                                                                                       | Verified text. Applying it to villages is interpretation, high confidence |
| Exceptions                                               | Only Art. 20(3): storage technology not available in Indonesia, judged by a committee                                                                                                                     | Verified                                                                  |
| Has PP 71/2019 been revised or replaced?                 | No. It is in force with no amendment recorded. A revision was in drafting as of September 2025, with data storage as one of its two main issues                                                           | Status verified. Drafting partly verified                                 |
| Implementing ministerial regulation                      | Permenkomdigi 5/2025, in force 25 March 2025. Requires storage in Indonesia, in the Pusat Data nasional, plus registration and data classification                                                        | Verified                                                                  |
| Does UU 27/2022 (PDP) change it?                         | No. It has no localisation rule. It allows transfers abroad under conditions, and keeps other data-protection rules in force where they do not conflict                                                   | Verified                                                                  |
| Does PP 33/2026 (PDP implementing PP) change it?         | No source reports a localisation rule or an amendment of PP 71/2019. Sources describe a conditional transfer regime only                                                                                  | Unverified (official text not located)                                    |
| Does the rule reach the vendor (processor)?              | Not directly, since the duty is the Village's. In practice yes: the Village can only comply if the vendor's VPS and backups are in Indonesia. Under PDP the vendor must follow the Village's instructions | Interpretation of verified texts                                          |

---

## 1. PP 71/2019 Art. 20: exact text

PP 71/2019 _tentang Penyelenggaraan Sistem dan Transaksi Elektronik_ (PSTE). Enacted 4 October 2019, promulgated 10 October 2019 (LN 2019 No. 185, TLN 6400), in force on promulgation (Art. 104). It repealed PP 82/2012 (Art. 103(2)).

- BPK page (status _Berlaku_, in force; no amendment recorded): <https://peraturan.bpk.go.id/Details/122030/pp-no-71-tahun-2019>
- Full text: <https://peraturan.bpk.go.id/Download/112816/PP%20Nomor%2071%20Tahun%202019.pdf>
- Komdigi JDIH copy: <https://jdih.komdigi.go.id/produk_hukum/view/id/695/t/peraturan+pemerintah+nomor+71+tahun+2019>

**Art. 20** (verified, transcribed from the BPK copy):

> (1) Penyelenggara Sistem Elektronik Lingkup Publik wajib memiliki rencana keberlangsungan kegiatan untuk menanggulangi gangguan atau bencana sesuai dengan risiko dari dampak yang ditimbulkannya.

Public-scope operators must have a business continuity plan for disruption or disaster, in line with the risk of its impact.

> (2) Penyelenggara Sistem Elektronik Lingkup Publik wajib melakukan pengelolaan, pemrosesan, dan/atau penyimpanan Sistem Elektronik dan Data Elektronik di wilayah Indonesia.

Public-scope operators must manage, process and/or store the electronic system and electronic data in Indonesian territory.

> (3) Penyelenggara Sistem Elektronik Lingkup Publik dapat melakukan pengelolaan, pemrosesan, dan/atau penyimpanan Sistem Elektronik dan Data Elektronik di luar wilayah Indonesia dalam hal teknologi penyimpanan tidak tersedia di dalam negeri.

They may manage, process and/or store the system and data outside Indonesia **if the storage technology is not available in Indonesia**.

> (4) Kriteria teknologi penyimpanan tidak tersedia di dalam negeri sebagaimana dimaksud pada ayat (3) ditentukan oleh komite yang terdiri atas kementerian yang menyelenggarakan urusan pemerintahan di bidang komunikasi dan informatika, lembaga yang membidangi urusan pengkajian dan penerapan teknologi, lembaga yang membidangi urusan keamanan siber, dan Kementerian atau Lembaga terkait.
>
> (5) Pembentukan komite sebagaimana dimaksud pada ayat (4) ditetapkan oleh Menteri.

A committee decides the criteria for "not available in Indonesia". It is made up of the communications ministry, the technology assessment agency, the cyber-security agency and the relevant ministries or agencies, and the Minister forms it.

> (6) Dalam hal Penyelenggara Sistem Elektronik Lingkup Publik menggunakan layanan pihak ketiga, Penyelenggara Sistem Elektronik Lingkup Publik wajib melakukan klasifikasi data sesuai risiko yang ditimbulkan.
>
> (7) Ketentuan lebih lanjut mengenai klasifikasi data sesuai risiko sebagaimana dimaksud pada ayat (6) diatur dengan Peraturan Menteri.

When a public-scope operator uses **third-party services**, it must classify its data by risk. A ministerial regulation sets the details (now Permenkomdigi 5/2025, section 3).

The Elucidation explains only para. (1) (business continuity plan). It says _cukup jelas_ (self-explanatory) for paras. (2) to (7).

**Contrast with private scope (Art. 21).** Private-scope operators _may_ store data in or outside Indonesia (Art. 21(1)). If they store it abroad, they must make sure supervision and law enforcement remain effective (Art. 21(2)) and must give access for those purposes (Art. 21(3)). The press summary behind #12 was right that the localisation duty applies to the public scope only.

**Related provisions (verified):**

- **Transition.** Public-scope operators already running in October 2019 had 2 years to comply with Art. 20(2) (Art. 102(2)). That period ended in October 2021.
- **Sanctions.** Art. 100(1) lists the provisions whose breach carries administrative sanctions. It lists Art. 21(2)–(3) but **not Art. 20**. PP 71/2019 itself therefore names no sanction for a public-scope operator that stores data abroad. The duty is still binding (_wajib_), and Komdigi supervises it (Permenkomdigi 5/2025 Art. 100).
- **Strategic data (Art. 99).** Institutions holding strategic electronic data, including the government administration sector, must keep electronic documents and backups linked to a designated data centre. UU 1/2024 (second amendment of UU ITE) restates this in Art. 40(3)–(4) and adds that other institutions keep backups "as their data-protection needs require" (Art. 40(5)). No source shows that village population data has been designated as strategic data. _Unverified_ either way.

## 2. A Village government is a public-scope operator

**Definitions in PP 71/2019 (verified):**

- _Penyelenggara Sistem Elektronik Lingkup Publik_ means an electronic system operated by an _Instansi Penyelenggara Negara_ (state institution) or by an institution it appoints (Art. 1 point 5). The public scope covers (a) _Instansi_ and (b) _institusi yang ditunjuk oleh Instansi_, an institution appointed by an _Instansi_ (Art. 2(3)).
- _Instansi_ means legislative, executive and judicial institutions at central and regional level, **and other institutions formed by legislation** (_instansi lain yang dibentuk dengan peraturan perundang-undangan_) (Art. 1 point 35).
- An appointed institution is one that operates the public-scope system **on behalf of** the appointing _Instansi_ (Elucidation of Art. 2(3)(b)).

**Permenkomdigi 5/2025 names the village government explicitly (verified):**

- Art. 1 point 29 defines _Pemerintah Desa_ as the village head, assisted by village officials, _sebagai unsur penyelenggara pemerintah daerah_ (as part of regional government).
- Art. 32(2) lists `.desa.id` as one of the _Nama Domain Instansi_ (state-institution domain names). Art. 32(4) reserves `.desa.id` for the official electronic address of a village government.
- Art. 34(1): the village government **must** use that domain for village government business.

**Interpretation (high confidence).** A village government is formed under UU 6/2014 and PP 16/2026. It fits "other institutions formed by legislation", and Komdigi's own regulation treats it as an _Instansi_. A Village running Pranata is therefore a public-scope operator, and Art. 20(2) applies to it. No official text says in one sentence that "a village government is a public-scope operator".

## 3. Permenkomdigi 5/2025 (implementing regulation for the public scope)

Permenkomdigi 5/2025 _tentang Penyelenggara Sistem Elektronik Lingkup Publik_. Enacted 18 March 2025, promulgated and in force 25 March 2025 (BN 2025 No. 225). It implements PP 71/2019 Art. 20(7) among others. It repealed Permenkominfo 5/2015 (state-institution domain registrars) and Permenkominfo 10/2015 (registration of state-institution electronic systems) (Art. 103).

- BPK page (status _Berlaku_): <https://peraturan.bpk.go.id/Details/317335/permenkomdigi-no-5-tahun-2025>
- Full text (BPK): <https://peraturan.bpk.go.id/Download/378675/Permenkomdigi%205%20tahun%202025.pdf>
- Komdigi copy: <https://domain.go.id/file-landing/PM-Komdigi-No-5-Tahun-2025.pdf>
- JDIH Komdigi abstract: <https://jdih.komdigi.go.id/produk_hukum/abstrak/524>

### Storage location (verified)

> **Art. 87(1)** PSE Lingkup Publik wajib melakukan pengelolaan, pemrosesan, dan/atau penyimpanan Data Terklasifikasi di wilayah Negara Kesatuan Republik Indonesia dengan memastikan kontrol dan tindakan keamanan yang sesuai.

Public-scope operators must manage, process and/or store classified data in Indonesia, with suitable security controls and measures.

- **Where (Art. 87(2)–(5)).** Data is held in the _Pusat Data nasional_ and/or at the _Instansi_.
  - _Open_ and _restricted_ data go in the _Pusat Data nasional_. It consists of government data-centre facilities, government cloud, and _layanan pihak ketiga yang terlibat dalam penyelenggaraan Pusat Data nasional_ (third-party services taking part in running the national data centre).
  - _Closed_ data goes in a government data-centre facility, or in encrypted storage in the _Instansi_'s own computing centre (Art. 89).
- **Exception (Art. 88).** Repeats PP 71/2019 Art. 20(3)–(5): storage abroad only if the storage technology is not available in Indonesia, as judged by a committee the Minister forms. No committee decision was found. _Unverified_ whether the committee exists.
- **Cloud (Art. 96).** Public-scope operators _harus memanfaatkan layanan Komputasi Awan yang diselenggarakan di Pusat Data nasional_ (must use cloud services run within the national data centre).
  - These are government cloud, or third-party cloud from providers **registered with Komdigi** that can be audited (Art. 96(2), (4), (6)).
  - A public-scope operator that uses a third-party cloud provider must classify its data (Art. 96(7)).
- **Registered-provider list (Art. 97).** The Minister publishes the list of registered third-party cloud providers.
  - Operators already using a third-party provider must move to a registered one within 1 year after the list is published, or when their current contract ends (Art. 102).
  - _Unverified_: whether the list has been published. None was found as of 2026-11-19.

### Data classification (verified)

- **Duty (Art. 62).** Every public-scope operator must classify its data by risk. The classification decides where the data may be managed, processed and stored.
- **Three levels (Art. 64):**
  - _terbuka_ (open): low risk;
  - _terbatas_ (restricted): medium risk;
  - _tertutup_ (closed): high risk.
- **Who decides (Art. 63, 73, 83).** Classified data follows the list set by the head of the _Instansi_ or the **regional head** under the One Data Indonesia rules. For regions, the regional head sets it by decree, following Ministry of Home Affairs guidelines.
- **Encryption.** Restricted and closed data must be stored encrypted at rest (Art. 90).
- **Moving data.** Restricted data sent over a network goes through the government intranet, encrypted in transit (Art. 91(2)). Restricted data sent off-network needs encryption provided by the cyber-security agency and sealed media (Art. 92(3)). Closed data may not be shared over a network or on media (Art. 91(3), 92(4)).
- **Retention and destruction (Art. 94–95).** The data-owning _Instansi_ sets retention, and must destroy data securely when retention ends. That includes data held by a third-party cloud provider (Art. 95(2)).

**Interpretation.** The regulation describes classification for central institutions and regional governments only. It does not describe classification by a village. Since it calls the village government _unsur penyelenggara pemerintah daerah_, the regency (_kabupaten/kota_) list most likely governs village data. Personal population data (NIK, family cards, children's data) would most likely be _terbatas_ or _tertutup_, never _terbuka_. _Unverified_: no regency classification decree covering village population data was checked.

### Registration (verified)

- **Duty (Art. 4).** Every public-scope operator must register each electronic system with Komdigi before users start using it. It must also secure the system, protect personal data and run a system feasibility test (_uji kelaikan_) (Art. 4(3)).
- **What is declared (Art. 12).** The registration includes:
  - the server's DNS name or IP address;
  - the risk category;
  - the classified data held;
  - the personal data processed;
  - _keterangan lokasi pengelolaan, pemrosesan, dan/atau penyimpanan_ (where the system and data are managed, processed and stored).
- **Sanction (Art. 16).** An unregistered system can be blocked (_access blocking_). Wrong or outdated registration data leads to written warnings and then suspension.
- **Transition (Art. 101).** Existing systems had 1 year to register and adjust, until 25 March 2026. That deadline has passed.

### Appointed institutions (verified)

An _Institusi_ appointed to operate a public-scope system must meet three criteria (Art. 3(3)–(4)):

- it holds a private-scope operator registration (_tanda daftar PSE Lingkup Privat_);
- it is an Indonesian legal entity;
- it **has a data centre located in Indonesia**.

The appointment must be an assignment _ditetapkan dengan peraturan perundang-undangan_, that is, made by a regulation, not by a contract alone.

## 4. Is PP 71/2019 being revised or replaced?

- **Status.** BPK lists PP 71/2019 as _Berlaku_ (in force). It records no amending or replacing regulation; the only relation shown is that it repealed PP 82/2012. Checked 2026-11-19. Verified.
- **Drafting.** Kemenko Polkam (Coordinating Ministry for Political and Security Affairs) press releases show the revision was still being drafted with Komdigi in 2025:
  - 28 August 2025 (SP No. 390/SP/HM.01.02/POLKAM/8/2025). The revision is driven by "field dynamics" and by UU 1/2024. Its two main issues are _penyimpanan data_ (data storage) and content moderation. <https://polkam.go.id/kemenko-polkam-gelar-rakor-bahas-revisi-pp-71-tahun-2019-tentang-pste/>
  - 25 September 2025 (SP No. 453/SP/HM.01.02/POLKAM/9/2025). It covers rules on "managing, processing and storing electronic data, both in Indonesia and abroad", and access for supervision and law enforcement. The government said storage abroad risks data sovereignty, while private platforms argued for "Data Free Flow with Trust". <https://polkam.go.id/bahas-revisi-pp-pste-pemerintah-tekankan-penguatan-keamanan-data-dan-akses-yang-proporsional/>
  - Partly verified: official press releases, not a draft text.
- **Direction.** Both releases concern private-scope platforms (the September meeting invited private-scope operators only). Nothing found suggests loosening the public-scope duty. _Unverified_: the content of any current draft. No 2026 update was found.

## 5. Personal data protection law and its implementing PP

### UU 27/2022 (PDP), verified

Source: <https://peraturan.bpk.go.id/Download/224884/UU%20Nomor%2027%20Tahun%202022.pdf>

- **No localisation rule.** Art. 56 lets a controller transfer personal data to a controller or processor outside Indonesia in three tiers:
  1. the destination country gives equal or higher protection (Art. 56(2));
  2. failing that, there are adequate and binding safeguards (Art. 56(3));
  3. failing both, the data subject consents (Art. 56(4)).
- **Other rules stay in force.** Art. 75: on entry into force, every existing provision on personal data protection _masih tetap berlaku sepanjang tidak bertentangan_ (stays in force where it does not conflict) with the law. PP 71/2019 Art. 20 is a location rule for public-scope systems, not a personal-data rule that conflicts with Art. 56. _Interpretation_: both apply, and a Village must meet both, so its data stays in Indonesia.
- **Processor duties.** A processor processes on the controller's instructions (Art. 51(1)), and that processing is the controller's responsibility (Art. 51(3)). Sub-processors need the controller's prior written approval (Art. 51(4)–(5)). Processing outside the controller's instructions becomes the processor's own responsibility (Art. 51(6)). The controller's security duties in Art. 35–39 also bind the processor (Art. 52).

### PP 33/2026 (PDP implementing regulation), unverified

- **Metadata.** Secondary sources now agree on the following:
  - Title: _Peraturan Pelaksanaan UU 27/2022 tentang Pelindungan Data Pribadi_.
  - Enacted and promulgated 16 July 2026 (LN 2026 No. 88, TLN 7190).
  - 225 articles in 12 chapters. This settles the article count that was disputed in the village-regulations note.
  - In force 16 January 2027 (Art. 225: six months after promulgation).
- **Sources.** Hukumonline Pro (<https://pro.hukumonline.com/a/lt6a97c3fe4527c/pemerintah-akhirnya-terbitkan-kerangka-implementasi-pelindungan-data-pribadi--berlaku-mulai-16-januari-2027>), an RSM Indonesia client alert of 18 September 2026 (<https://www.rsm.global/indonesia/sites/default/files/media/publications/Client%20Alert/2026/RSM%20Indonesia%20Client%20Alert%20-%20PP%20332026%20Six%20Implementation%20Priorities%20before%2016%20January%202027.pdf>), and Meridian Hukum (<https://meridianhukum.com/peraturan/pp-no-33-tahun-2026>).
- **Not found officially.** The text is not on peraturan.bpk.go.id or peraturan.go.id (the peraturan.go.id URL returns 404). It was not found on the JDIH Setneg or Komdigi sites either.
- **What sources report on transfers and processors:**
  - International transfers follow the UU PDP order: equivalent protection, then binding safeguards, then consent (reported as Art. 165).
  - Processor engagement needs a written agreement with minimum terms (reported as Art. 14).
  - Sub-processors need the controller's prior written approval and equivalent protection (reported as Art. 15).
- **What no source reports.** No source mentions a data-localisation rule, or any amendment or repeal of PP 71/2019. Even if the official text confirms this, it would not lift the public-scope duty in PP 71/2019 Art. 20 and Permenkomdigi 5/2025 Art. 87.

## 6. Does the duty reach the vendor?

**The duty is the Village's.** PP 71/2019 Art. 20(2) and Permenkomdigi 5/2025 Art. 87(1) bind the _PSE Lingkup Publik_, which is the Village.

**The vendor is not itself a public-scope operator** (interpretation).

- It would be one only as an appointed _Institusi_. That requires an appointment made by regulation, a private-scope registration, an Indonesian legal entity and its own data centre in Indonesia (Permenkomdigi 5/2025 Art. 3(3)–(4)).
- A vendor hired by contract is better described as a _Penyedia_, defined as a business supplying goods or services under a contract (Art. 1 point 37), or as a "third-party service" in PP 71/2019 Art. 20(6).
- If a regency or Village ever formally appointed the vendor as an _Institusi_, the vendor would carry the public-scope duties directly. It would also need to own a data centre in Indonesia.

**In practice the duty passes to the vendor:**

- **Location.** The Village can only meet Art. 20(2) and Art. 87 if the system it uses runs in Indonesia. That covers the VPS, the database, file storage and backups.
- **Instructions.** Under UU 27/2022 the vendor is the Village's processor. It must process only on the Village's instructions (Art. 51(1)). Storing data abroad against those instructions makes the processing the vendor's own responsibility (Art. 51(6)).
- **Third-party services.** Using third-party services triggers the Village's duty to classify its data (PP 71/2019 Art. 20(6); Permenkomdigi 5/2025 Art. 96(7)).
- **Hosting providers.** The VPS host and the backup storage provider are sub-processors. They need the Village's prior written approval (UU 27/2022 Art. 51(5)).
- **Registered cloud.** Permenkomdigi 5/2025 Art. 96(1) on a strict reading wants the cloud to be part of the _Pusat Data nasional_. That means government cloud or a Komdigi-registered third-party provider. An ordinary commercial VPS in Indonesia satisfies the location rule, but may not satisfy the "registered provider" rule once Komdigi publishes the list (Art. 97, 102).

## Implications for Pranata

- **Keep the #12 constraint.** The VPS, PostgreSQL data, uploaded files and restic backups for every Installation stay in Indonesian data centres. The basis is now verified: PP 71/2019 Art. 20(2) and Permenkomdigi 5/2025 Art. 87(1). The "technology not available" exception cannot be relied on for commodity hosting.
- **Prefer Komdigi-registered cloud providers.** When choosing the VPS and object-storage providers (ops repo), prefer providers registered as third-party cloud providers for the _Pusat Data nasional_. Track the Art. 97 list. Once it is published, existing Installations have 1 year (or until contract end) to sit on a registered provider (Art. 102). Write that migration into the hosting contracts.
- **Help the Village register.** The Village must register its Pranata system with Komdigi before residents use it (Art. 4). Registration declares server DNS or IP, risk category, classified data, personal data processed and storage location (Art. 12). Provide a filled-in registration data sheet per Installation. Re-file it whenever the server, IP or provider changes (Art. 14, 16).
- **`.desa.id` domain.** A Village must use its `.desa.id` domain for village government business (Permenkomdigi 5/2025 Art. 34). The public site and admin should run on the Village's `.desa.id` name, or a subdomain of it. Do not use a vendor domain except for the demo Installation.
- **Encryption at rest.** Population data will likely be _terbatas_ or _tertutup_. Art. 90 requires restricted and closed data to be encrypted at rest. restic backups are already encrypted client-side. Add encryption at rest for the VPS volume holding PostgreSQL and files, for example full-disk or volume encryption, and record it in the DPIA.
- **Classification input.** Ship a default data classification for Pranata's record types, e.g. public site content _terbuka_, aggregate statistics _terbuka_, resident and family records _terbatas_ at least. The Village (or its regency's decree) confirms it. This also feeds the registration form and the DPIA.
- **No personal data abroad.** Keep the #12 rules that logs, monitoring, uptime checks and dead-man's-switch pings carry no personal data. Any third-party service outside Indonesia must never receive Village data, for example error trackers, email or SMS gateways, or AI services. Email or WhatsApp notifications to residents would carry personal data, so their providers need checking before such a feature ships.
- **Exports.** Strict reading of Art. 92(3): restricted data sent off-network needs cyber-security-agency encryption and sealed media. Village staff exporting spreadsheets of resident data is a gap. Keep exports logged and minimised, and revisit this when the Population module's export feature is designed.
- **Contracts.** The vendor–Village processing agreement should state:
  - all processing and storage stays in Indonesia;
  - the hosting and backup providers are named sub-processors, approved in writing;
  - the vendor supports the Village's PSE registration and data classification;
  - data is destroyed at third-party providers when the contract ends (Art. 95(2)).

## Open questions and unverified items

1. **PP 33/2026 official text.** Confirm its existence, effective date, the transfer articles (reported as Art. 165) and the processor articles (reported as Art. 14–15). Confirm it contains no localisation rule and does not amend PP 71/2019.
2. **Registered third-party cloud list** (Permenkomdigi 5/2025 Art. 97). Has Komdigi published it, and which Indonesian VPS and object-storage providers are on it?
3. **Pusat Data nasional guidelines** (Art. 96(8)). Do they let a village use a commercial registered provider directly, or only through government channels?
4. **Village data classification.** Which regency decree, or which Ministry of Home Affairs guideline under Art. 83(2), sets the classification for village population data?
5. **PP 71/2019 revision.** Watch Komdigi and Kemenko Polkam for a draft or an enacted replacement, and check whether the public-scope rule in Art. 20 changes.
6. **Strategic data.** Has the government designated village or population data as strategic electronic data (PP 71/2019 Art. 99, UU ITE Art. 40(3)–(4))? If so, backups must also link to a designated data centre.
7. **Art. 20(4) committee.** Has the committee been formed, and has it ever issued criteria? Low priority, since Pranata does not need the exception.
