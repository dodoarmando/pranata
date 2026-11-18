# Regulations for village records and letters

Research note for [#4](https://github.com/dodoarmando/pranata/issues/4). As of 2026-11-18.

This note covers what Indonesian regulations require of a system that keeps a village's administration books, region hierarchy, service letters, monthly population recap, Indeks Desa data, public website and personal data. Each section gives the requirement, its source (regulation and article), and what it means for Pranata.

Labels used below:

- **Verified**: read in the official text (peraturan.go.id, peraturan.bpk.go.id, or a copy published by the issuing body).
- **Partly verified**: the official source confirms the regulation exists or its status, but the detail comes from a secondary source.
- **Unverified**: secondary sources only. Check before building on it.

## Summary

| Area                     | Current rule                                                                                                                                    | Status                                                                                 |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Village law              | UU 6/2014, last amended by UU 3/2024                                                                                                            | Verified                                                                               |
| Implementing regulation  | **PP 16/2026** (in force 27 March 2026). It repeals PP 43/2014, PP 47/2015 and PP 11/2019                                                       | Verified                                                                               |
| Administration books     | Permendagri 47/2016 (_Administrasi Pemerintahan Desa_). No replacement found                                                                    | Text verified. Status partly verified                                                  |
| Region codes             | Permendagri 58/2021. Code data in Kepmendagri 300.2.2-2138/2025, amended by 300.2.2-2430/2025                                                   | Format verified. Kepmendagri numbers partly verified                                   |
| Dusun, RW, RT            | PP 16/2026 Art. 65 (_pelaksana kewilayahan_) and Art. 169 (RT/RW as LKD)                                                                        | Verified                                                                               |
| Letter numbering         | No national format for village letters. Permendagri 83/2022 classification codes are the basis for numbering in central and regional government | Verified that no national village format exists. The local format must be configurable |
| Monthly population recap | Permendagri 47/2016 Art. 6(3) and Lampiran B.3                                                                                                  | Verified                                                                               |
| Indeks Desa              | Permendesa PDTT 9/2024 (replaced the IDM in Permendesa 2/2016)                                                                                  | Verified                                                                               |
| Public information       | UU 14/2008 (KIP); Perki 1/2018 (_Standar Layanan Informasi Publik Desa_); PP 16/2026 Art. 129                                                   | Verified. Relation of Perki 1/2018 to Perki 1/2021 unverified                          |
| Personal data            | UU 27/2022 (PDP); UU 24/2013 (Adminduk) Art. 79, 84, 86, 95A                                                                                    | Verified                                                                               |
| PDP implementing PP      | PP 33/2026, reportedly enacted 16 July 2026 and in force 16 January 2027                                                                        | **Unverified** (secondary sources only)                                                |

---

## 1. Village law framework

**Requirement.** The village head must run village administration well and give information to residents:

- _menyelenggarakan administrasi Pemerintahan Desa yang baik_ (run village administration well): UU 6/2014 Art. 26(4)(i), as amended by UU 3/2024.
- _memberikan informasi kepada masyarakat Desa_ (give information to village residents): Art. 26(4)(q).
- Each year the village head must publish written information on how the village was governed: Art. 27(a).
- The village information system (_Sistem Informasi Desa_) is run by the village government and open to local residents: Art. 86(5).

**Implementing regulation.** PP 16/2026 replaced PP 43/2014 and its amendments in full. It took effect on 27 March 2026 (Art. 187–188, LN 2026 No. 36). It has no clause keeping older ministerial regulations in force. Several topics are left to new ministerial regulations, including village administration staffing (Art. 66(2)) and village financial systems (Art. 128(3)). Ministerial regulations issued under PP 43/2014 may therefore be replaced over 2026–2027.

- UU 3/2024: <https://peraturan.bpk.go.id/Details/283617/uu-no-3-tahun-2024>
- PP 16/2026 (metadata and repeal status): <https://peraturan.bpk.go.id/Details/349409/pp-no-16-tahun-2026>
- PP 16/2026 full text: <https://peraturan.bpk.go.id/Download/413094/PP%20Nomor%2016%20Tahun%202026.pdf>

**Implications.** Where possible, cite UU 6/2014 as amended and PP 16/2026 in product copy and docs, not PP 43/2014. Model the book formats as data (column definitions), not hard-coded layouts, so that a new Permendagri on administration can be absorbed.

## 2. Village administration books

**Current rule.** Permendagri 47/2016 _tentang Administrasi Pemerintahan Desa_ (BN 2016 No. 1100). It replaced Permendagri 32/2006 (Art. 14).

- No amendment or replacement was found on BPK, peraturan.go.id or the wider web.
- pasal.id lists it as in force.
- Its legal basis included PP 43/2014, which PP 16/2026 has now repealed. Even so, no replacement Permendagri was found.
- _Partly verified_: the BPK status page for this Permendagri could not be located.

Official text, including the Lampiran with the forms: <https://peraturan.go.id/files/bn1100-2016.pdf>. The Lampiran pages are scanned images. The columns below were transcribed by reading them.

### Books by category

| Category                                 | Books                                                                                                                                                                                                                                                                                                                                                                                                                  | Article   |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| _Administrasi Umum_ (general)            | A.1 _Peraturan di Desa_ (village regulations); A.2 _Keputusan Kepala Desa_ (village head decrees); A.3 _Inventaris dan Kekayaan Desa_ (inventory and assets); A.4 _Aparat Pemerintah Desa_ (village officials); A.5 _Tanah Kas Desa_ (village-owned land); A.6 _Tanah di Desa_ (land in the village); A.7 _Agenda_ (letter log); A.8 _Ekspedisi_ (dispatch log); A.9 _Lembaran Desa dan Berita Desa_ (village gazette) | Art. 5(2) |
| _Administrasi Penduduk_ (population)     | B.1 _Induk Penduduk_ (resident register); B.2 _Mutasi Penduduk Desa_ (mutation register); B.3 _Rekapitulasi Jumlah Penduduk_ (population recap); B.4 _Penduduk Sementara_ (temporary residents); B.5 _Kartu Tanda Penduduk dan Kartu Keluarga_ (ID-card and family-card book)                                                                                                                                          | Art. 6(2) |
| _Administrasi Keuangan_ (finance)        | APB Desa; RAB (cost plan); _Kas Pembantu Kegiatan_ (activity sub-cash); _Kas Umum_ (general cash); _Kas Pembantu_ (cash sub-book); _Bank Desa_ (bank book)                                                                                                                                                                                                                                                             | Art. 7    |
| _Administrasi Pembangunan_ (development) | RKP Desa; development activities; inventory of development results; community cadres                                                                                                                                                                                                                                                                                                                                   | Art. 8    |
| _Administrasi Lainnya_ (other)           | BPD records, village deliberations, community institutions. Format set by Perbup/Perwali (regency or city regulation)                                                                                                                                                                                                                                                                                                  | Art. 9    |

Every book page carries a signature block: "MENGETAHUI KEPALA DESA" (acknowledged by the village head) and the village secretary (_Sekretaris Desa_).

### Columns of the books Pranata touches

**B.1 _Buku Induk Penduduk_** (resident register), 16 columns:

1. _nomor urut_: sequence number, one page per family card.
2. _nama lengkap/panggilan_: full name and nickname.
3. _jenis kelamin_: sex, L or P.
4. _status perkawinan_: marital status, coded K (married), BK (not married), JD (widow), DD (widower).
5. _tempat lahir_: place of birth.
6. _tanggal lahir_: date of birth.
7. _agama_: religion (_agama dan kepercayaan_).
8. _pendidikan terakhir_: last education, formal or informal.
9. _pekerjaan_: occupation.
10. _dapat membaca huruf_: scripts the person can read, coded L (Latin), D (regional), A (Arabic), AL, AD or ALD.
11. _kewarganegaraan_: citizenship, WNI or WNA.
12. _alamat lengkap_: full address.
13. _kedudukan dalam keluarga_: position in family, coded KK (head), Ist (wife), AK (biological child), AA (adopted child), Pemb (helper).
14. NIK (national ID number).
15. _nomor KK_: family card number.
16. _keterangan_: notes.

**B.2 _Buku Mutasi Penduduk Desa_** (mutation register), kept per month (_bulan … tahun …_), 13 columns:

- 1 number; 2 name; 3–4 place and date of birth; 5 sex; 6 citizenship.
- _Penambahan_ (additions): 7 _datang dari_ (arrived from); 8 date of arrival.
- _Pengurangan_ (removals): 9 _pindah ke_ (moved to); 10 date of move, as on the move certificate; 11 place of death; 12 date of death.
- 13 notes.

**B.3 _Buku Rekapitulasi Jumlah Penduduk_** (population recap), kept per month, one row per _dusun/lingkungan_, 33 columns:

- 1 number; 2 _nama dusun/lingkungan_ (hamlet name).
- _Awal bulan_ (start of month): 3–6 WNA L, WNA P, WNI L, WNI P; 7 number of family heads (_KK_); 8 number of family members; 9 _jumlah jiwa_ (total people), defined as 7 + 8.
- _Tambahan bulan ini_ (additions this month): 10–13 _lahir_ (births) by WNA/WNI × L/P; 14–17 _datang_ (arrivals) by WNA/WNI × L/P.
- _Pengurangan bulan ini_ (removals this month): 18–21 _meninggal_ (deaths) by WNA/WNI × L/P; 22–25 _pindah_ (moves out) by WNA/WNI × L/P.
- _Akhir bulan_ (end of month): 26–29 WNA/WNI × L/P; 30 _KK_; 31 family members; 32 total people (30 + 31).
- 33 notes.

**B.4 _Buku Penduduk Sementara_** (temporary residents), kept per year, for short-stay visitors, 15 columns:

- 1 number; 2 name; 3–4 sex (L/P ticks); 5 identity number or document.
- 6 place and date of birth or age; 7 occupation.
- 8 _kebangsaan_ (WNI) and 9 _keturunan_ (country of origin, for WNA).
- 10 arrived from; 11 purpose of visit; 12 name and address of the host.
- 13 arrival date; 14 departure date; 15 notes.

**B.5 _Buku KTP dan Buku KK_** (ID-card and family-card book), 19 columns:

- 1 number; 2 _no. KK_; 3 full name as on the ID card; 4 NIK; 5 sex.
- 6 place and date of birth; 7 _golongan darah_ (blood type); 8 religion; 9 education; 10 occupation.
- 11 address including RT/RW; 12 marital status.
- 13 place and date the e-KTP was issued.
- 14 _status hubungan keluarga_ (family relationship); 15 citizenship.
- 16 father's name; 17 mother's name.
- 18 _tanggal mulai tinggal di desa_ (date the person started living in the village).
- 19 notes.

**A.7 _Buku Agenda_** (letter log), 11 columns. One running sequence covers both incoming and outgoing letters:

- 1 number; 2 date received or sent.
- _Surat masuk_ (incoming): 3 number; 4 date; 5 sender; 6 subject.
- _Surat keluar_ (outgoing): 7 number; 8 date; 9 addressee; 10 subject.
- 11 notes.

**A.8 _Buku Ekspedisi_** (dispatch log), 6 columns: 1 number; 2 dispatch date; 3 letter date and number; 4 subject; 5 addressee; 6 notes.

**A.1 _Buku Peraturan di Desa_** (village regulations), 10 columns:

- 1 number; 2 type (_Perdes_, _Peraturan Bersama_, _Perkades_); 3 number and date enacted; 4 title; 5 summary.
- 6 date of agreement with the BPD (village regulations only); 7 date reported to the Bupati.
- 8 _Lembaran Desa_ number and date; 9 _Berita Desa_ number and date.
- 10 notes.

Columns of A.2–A.6, A.9 and the finance and development books were not transcribed. They are in the same Lampiran, pages 13–37.

### Implications

- **Resident and family model.** Store every field the books need: the B.1 fields plus the extra B.5 fields (blood type, e-KTP issue place and date, parents' names, start date of residence, RT/RW).
- **Marital status codes.** The book's codes (K/BK/JD/DD) are older than the four categories Dukcapil uses (_belum kawin_, _kawin_, _cerai hidup_, _cerai mati_). Store the Dukcapil value and map it to K/BK/JD/DD only when printing B.1. The exact Dukcapil value list is not verified in this note.
- **Mutation events.** Record mutations as typed, dated events: _lahir_ (birth), _datang_ (arrival), _pindah_ (move out), _meninggal_ (death). B.2 has no birth columns, but B.3 counts births. Generate B.2 as a monthly view of the events, not as a separately edited table.
- **Monthly recap.** Generate B.3 per _dusun_ from the events plus an opening balance. This needs a population snapshot that can be computed as of any month end.
- **Temporary residents.** B.4 records visitors, not residents. Keep them as a separate, lightweight record and out of the resident counts.
- **Letter log.** The Letters module should produce the outgoing half of A.7 and A.8. Incoming-letter registration is optional, but the Agenda numbers incoming and outgoing letters in one sequence.
- **Printing.** Each printed book needs the village head and village secretary signature block.

## 3. Region hierarchy (desa, dusun, RW, RT)

**Requirement.**

- **Government region codes.** Permendagri 58/2021 Art. 4(3) sets the format:
  - 2 digits for the province.
  - 2 for the regency or city: 01–69 for a regency, 71–99 for a city (Art. 4(3)(b), Art. 6).
  - 2 for the district (_kecamatan_).
  - 4 for the village. The first of the 4 digits is **1** for a _kelurahan_ (urban ward), **2** for a _desa_ (village) and **3** for a _desa adat_ (customary village) (Art. 4(3)(d)).
  - Written with dots, e.g. `PP.KK.CC.2NNN`.
  - Source: <https://peraturan.go.id/files/bn1391-2021.pdf>
- **Current code list.** Kepmendagri 300.2.2-2138/2025, amended by Kepmendagri 300.2.2-2430/2025 (in force 23 June 2025). It replaced Kepmendagri 100.1.1-6117/2022.
  - _Partly verified_: confirmed by Kemendagri news coverage and legal databases (<https://paralegal.id/peraturan/keputusan-menteri-dalam-negeri-nomor-300-2-2-2138-tahun-2025/>), but not by a Kemendagri-hosted copy. No 2026 Kepmendagri was found.
- **Below the village.** National codes stop at the village.
  - The _dusun_ (hamlet) is a territorial unit. Its head, the _kepala kewilayahan/kepala dusun_, is one of the village officials, as _pelaksana kewilayahan_ (territorial staff): PP 16/2026 Art. 65(1)(b) and Elucidation of Art. 90(1).
  - RT (_rukun tetangga_) and RW (_rukun warga_) are community institutions (LKD), formed at the initiative of the village government and residents: PP 16/2026 Art. 169(2)(a)–(b). Their names vary by region (_sebutan lain_).
  - Detailed LKD rules are in Permendagri 18/2018. Sources suggest it is still in force (one article revoked by Permendagri 13/2024). _Unverified_: whether it was replaced after PP 16/2026.

**Implications.**

- Store the 10-digit village code as the village's official identifier, and validate the format: digits only, and the 7th digit is 2 or 3 for a _desa_.
- Below the village, use a configurable tree: _dusun_ → RW → RT. The label is configurable, since regions use _dusun_, _banjar_, _jorong_, _lingkungan_ and others. Some villages have no RW level, so the levels must be optional.
- Group the monthly recap (B.3) by _dusun_, so every resident address must resolve to a _dusun_.
- Ship a seed of the national code list (province to village) from the current Kepmendagri. It is only needed to select and validate the Installation's own village and its parents.

## 4. Service-letter numbering

**Finding.** No national regulation sets the number format of village letters.

- Permendagri 1/2023 on official documents (_Tata Naskah Dinas_) covers **regional governments only**. For numbering it refers to other regulations (Art. 26): <https://peraturan.bpk.go.id/Download/301556/Permendagri%20Nomor%201%20Tahun%202023.pdf>
- Permendagri 83/2022 sets the archive classification codes (_Kode Klasifikasi Arsip_) for the Ministry of Home Affairs and regional governments. Art. 3(6): the codes are _dasar penomoran surat_ (the basis for numbering letters), as well as for filing, retention and retrieval. Its Lampiran has a "100.3 PEMERINTAH DESA" group and "400.8 KEPENDUDUKAN" (population). It revoked the older code list in Permendagri 78/2012 Lampiran C. Many village templates still use those old codes, e.g. 470 for population. Source: <https://peraturan.bpk.go.id/Details/247841/>
- In practice, the regency sets the village format by Perbup (regency regulation on village official documents or archive codes). A common pattern is `classification code / sequence number / village unit code / month (often in Roman numerals) / year`. _Unverified_ as a rule. It is common practice only.

**Implications.**

- Make the letter number a per-Installation template with tokens: classification code, sequence, unit code, month (Arabic or Roman numerals), year. Each letter type carries its own default classification code.
- The sequence must be gap-free and unique per year. Optionally it can be shared with incoming letters, to match the single A.7 sequence. Allocate it in a database transaction, so that two clerks issuing letters at once cannot get the same number.
- Seed the classification codes from the Permendagri 83/2022 Lampiran, and let the village override them, since many regencies still use the 2012 codes.

## 5. Monthly population recap

**Requirement.** The village head must report the population recap (B.3) to the Bupati/Walikota, through the _camat_ (district head), at the end of every month, on the recap form: Permendagri 47/2016 Art. 6(3). Verified.

The form is the B.3 table in section 2: per _dusun_, opening counts by citizenship and sex plus family heads and members, births, arrivals, deaths, moves, and closing counts.

**Implications.**

- Generate a B.3 report for any month, export it to PDF (Typst) and spreadsheet, and lock it once sent. Store the sent version, because later corrections to past events must not silently change a report that was already submitted.
- Show a reconciliation check: opening balance + additions − removals = closing balance.
- Note an oddity in the form: _jumlah jiwa_ is defined as KK + family members, so family members must exclude the head. Follow the form literally and document the rule in the UI.

## 6. Indeks Desa

**Current rule.** Permendesa PDTT 9/2024 _tentang Indeks Desa_, in force 18 October 2024. It revoked the _Indeks Desa Membangun_ (Permendesa PDTT 2/2016). No amendment was found as of November 2026.

- BPK page: <https://peraturan.bpk.go.id/Details/307189/permendesa-pdtt-no-9-tahun-2024>
- Full text: <https://peraturan.bpk.go.id/Download/367582/Permendesdtt%20Nomor%209%20Tahun%202024.pdf>

**Requirement.**

- **Six dimensions** (Art. 2): basic services, social, economic, environment, accessibility, and village governance (_tata kelola pemerintahan Desa_).
- **Annual data collection** (Art. 13(2)): March to June each year.
  - A village team includes the village government and the BPD, and is set by village head decree (Art. 14).
  - Results are validated in a village meeting, recorded in minutes, entered into the ministry's system, and reported to the _camat_ (Art. 15).
  - A village that skips data collection gets a written warning. After two years in a row, development programmes are deferred (Art. 16).
- **Status bands** (Art. 26), set by ministerial decree (Art. 25(3)):

  | Status                                    | Score        |
  | ----------------------------------------- | ------------ |
  | _Sangat Tertinggal_ (very underdeveloped) | 0–49.48%     |
  | _Tertinggal_ (underdeveloped)             | 49.49–57.38% |
  | _Berkembang_ (developing)                 | 57.39–69.34% |
  | _Maju_ (advanced)                         | 69.35–79.62% |
  | _Mandiri_ (self-reliant)                  | 79.63–100%   |

- **Governance indicators that Pranata directly affects** (Art. 8 and Lampiran, Dimension VI):
  - Indicator 44: service days. Fewer than working days = 1, working days only = 3, every day = 5.
  - Indicator 45, _Pemanfaatan Teknologi dalam Pelayanan Desa_ (use of technology in village services): four yes/no items worth 1 or 5 each:
    - _publikasi informasi_ (publishing information);
    - _pelayanan administrasi_ (administrative services);
    - _pelayanan pengaduan_ (complaint handling);
    - _pelayanan lainnya_ (other services).
  - Indicator 46: number of village meetings per year, and whether residents attend.

**Implications.**

- The website, online letter requests and a complaints channel each move indicator 45 from 1 to 5. That is a concrete selling point. Treat a complaints or feedback channel as a v1 candidate.
- Online letter requests available every day can support the "every day" score on indicator 44.
- The public site can show the village's current Indeks Desa score and status, entered by hand. There is no live integration (see the map's scope).
- Optional: a page that lists the village's own Indeks Desa answers per year, to support the March–June data collection. Data comes from village records; nothing is sent to the ministry.

## 7. Public-information duties of the village website

**Requirement.**

- **UU 14/2008 (KIP).** Public bodies must build an information and documentation system so that information is easy to reach (Art. 7(3)). They must publish periodic information (Art. 9): the body itself, its activities and performance, and financial reports, at least every 6 months (Art. 9(3)). Exempt information includes personal secrets such as family history and health (Art. 17(h)). Source: <https://peraturan.go.id/id/uu-no-14-tahun-2008>
- **Perki 1/2018 _Standar Layanan Informasi Publik Desa_** (BN 2018 No. 1899). Copy from a provincial information commission: <https://komisiinformasi.bantenprov.go.id/upload/putusanlainnya/Perki_1_2018.pdf>
  - _Berkala_ (periodic, Art. 2(1)), at least once a year (Art. 2(2)):
    - (a) village profile: address, vision and mission, duties, structure, short profiles of officials;
    - (b) table of current programmes, with schedule, person in charge, funding source and amount;
    - (c) table of incoming programmes from central, provincial, regency and third parties, **including data on aid recipients**;
    - (d) RPJM Desa, RKP Desa and the draft budget plan;
    - (e) the current-year APB Desa regulation;
    - (f) year-end or end-of-term governance report;
    - (g) financial report: budget realisation, activity realisation, unfinished activities, remaining budget, complaint address;
    - (h) list of regulations and **draft** regulations;
    - (i) how to request information.
  - _Serta merta_ (immediate, Art. 3): disasters, epidemics, food poisoning, planned utility outages and similar. At minimum on the village notice board or media residents commonly use. The announcement must cover the hazard, who is affected, evacuation, how to get help, and so on (Art. 3(2)).
  - _Setiap saat_ (always available, Art. 4):
    - (a) _Daftar Informasi Publik_ (public information register), with summary, owner, date and place made, format, and **retention period**;
    - (b) regulations and decisions together with supporting studies, meeting minutes, drafts and stages;
    - (d) profiles of the village head and officials;
    - (f) agreements with third parties;
    - (j) minutes of BPD meetings and village deliberations;
    - (m)–(o) BUM Desa documents;
    - and more.
  - Village duties (Art. 7): a village regulation on information openness, a notice board and information desk, and an up-to-date public information register. Appoint a village information officer, the PPID Desa (Art. 8).
  - Requests (Art. 13): may be written or not. The PPID records the requester's name and address. A registration number is given at once (also for email requests). Written reply within **10 working days**, extendable by **7 working days**. Objections and escalation to the information commission follow.
  - _Unverified_: whether Perki 1/2021 (general public information service standard) replaced Perki 1/2018 for villages. Village regulations issued in 2023–2024 still cite Perki 1/2018.
- **PP 16/2026 Art. 129(5)–(6).** At minimum, the village must publish regularly: the planned and enacted APB Desa, revenue sources, activity details and allocations, budget realisation including cashless transactions, the financial accountability report, and tax reports. It must publish them through, among others, _media digital resmi milik Pemerintah Desa_ (the village government's official digital media). Residents may object through the BPD (Art. 129(7)).
- **PP 16/2026 Art. 51(a) and 52.** End-of-year governance information must be published through media residents can easily reach.

**Implications.**

- The CMS needs structured content types (or fixed sections) for each Perki 1/2018 Art. 2 item, plus an APB Desa section that meets PP 16/2026 Art. 129(5).
- Add a **public information register** page with a retention-period field.
- Add a **regulations page** that lists drafts as well as enacted regulations. It can be fed from the A.1 book data.
- Add an **emergency announcement** banner for _serta merta_ information.
- Optional: an **information request** form with an automatic registration number and a 10 + 7 working-day deadline tracker.
- **Conflict with PDP.** Item (c) asks for aid-recipient data. Publishing it collides with PDP (section 8). Default to aggregated or minimised lists, never NIK, KK number or address. Let the village decide on names, with a warning.

## 8. Personal data protection (PDP) for a system holding NIK and family-card data

**Current rule.** UU 27/2022 _tentang Pelindungan Data Pribadi_, in force since 17 October 2022. The 2-year adjustment period ended 17 October 2024 (Art. 74).

- BPK page: <https://peraturan.bpk.go.id/Details/229798/uu-no-27-tahun-2022>
- Full text: <https://peraturan.bpk.go.id/Download/224884/UU%20Nomor%2027%20Tahun%202022.pdf>

**Implementing PP.** PP 33/2026 on implementing UU 27/2022 is _unverified_.

- Several legal-commentary sites report it was enacted on 16 July 2026 (LN 2026 No. 88) and takes effect on 16 January 2027.
- They report that it covers DPIA, DPO appointment, breach handling, transfers and the PDP agency, and that a draft Perpres creating the PDP agency was still in progress in mid-2026.
- The sources disagree on its length (120 vs 225 articles), and the official text was not located on BPK or peraturan.go.id. Example source: <https://justisio.com/regulation/indonesia/dbwgcly3epltypka7sebq61yx>
- Re-check before the Population module ships.

### Roles

- The **village government** is the data controller (_Pengendali Data Pribadi_).
- The **vendor** operating the Installation is the data processor (_Prosesor Data Pribadi_). It processes only on the controller's instructions (Art. 51(1)) and must get the controller's written approval before using sub-processors, such as a VPS host (Art. 51(4)–(5)). Processing outside the controller's instructions becomes the processor's own responsibility (Art. 51(6)).

### Obligations (UU 27/2022, verified)

| Obligation                                                                                                                                                                                      | Article        | Product implication                                                                                                                                                                 |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Child data (_data anak_) is **specific** personal data. So are health data, biometrics and criminal records                                                                                     | Art. 4(2)      | The population register always holds children, so it holds specific data                                                                                                            |
| A lawful basis is required. Options include legal obligation (c) and public-service or official-authority tasks (e)                                                                             | Art. 20        | Base population records and letters on (c) and (e), not consent. Use consent (Art. 20(2)(a), 21) only for optional extras, such as a resident's phone number used for notifications |
| Correct data within 3×24 hours of a request, and tell the subject                                                                                                                               | Art. 30        | An edit workflow with a timestamped request log                                                                                                                                     |
| **Record all processing activities**                                                                                                                                                            | Art. 31        | An append-only audit log of every create, read (of individual records), update, delete, export and print of personal data                                                           |
| Give subjects access to their data **and its processing history** within 3×24 hours                                                                                                             | Art. 32        | Must be able to produce one person's data plus their audit-log entries                                                                                                              |
| **DPIA** is mandatory for high-risk processing, including specific data and large scale                                                                                                         | Art. 34        | The village needs a DPIA. Ship a DPIA template filled in for Pranata's standard processing                                                                                          |
| Security, confidentiality, prevention of unauthorised access                                                                                                                                    | Art. 35–39     | Role-based access, encryption in transit and at rest, backups, least-privilege staff roles                                                                                          |
| Delete data when no longer needed, consent is withdrawn, the subject asks, or it was obtained unlawfully                                                                                        | Art. 43        | Soft-delete plus a purge path. Statutory books still need history, so define retention per record type                                                                              |
| **Destroy** data when its retention ends under the archive retention schedule (_jadwal retensi arsip_)                                                                                          | Art. 44(1)(a)  | A per-type retention setting. _Unverified_: the retention schedule for village population records                                                                                   |
| **Breach notice in writing within 3×24 hours** to the subjects and the PDP agency, covering what data was exposed, when and how, and the remedial steps. Sometimes the public must also be told | Art. 46        | An incident runbook. The vendor (processor) must alert the village at once. Notices need contact details, but the law gives no exemption for missing contact details                |
| Appoint a **DPO** (_pejabat/petugas pelindungan data pribadi_) when processing is for public services                                                                                           | Art. 53(1)(a)  | Each village must name one. Add a "DPO contact" setting shown on the public privacy page                                                                                            |
| Administrative sanctions for breaching the obligations above: written warning, suspension of processing, deletion or destruction of data, or a fine of up to 2% of annual revenue or receipts   | Art. 57(2)–(3) | —                                                                                                                                                                                   |

### Population-data law (UU 24/2013, amending UU 23/2006 Adminduk; verified)

Source: <https://peraturan.go.id/id/uu-no-24-tahun-2013>

- Art. 79(1): individual data and population documents must be stored and kept confidential.
- Art. 79(3) and 86(1a): officials and users given access must not disseminate population data or personal data beyond their authority. Art. 95A: unlawful dissemination carries up to 2 years in prison and/or a Rp25 million fine.
- Art. 84(1): protected personal data includes physical or mental disability, fingerprints, iris, signature, and other data that would shame a person (_aib_).
- Art. 79A and 95B: issuing population documents is free of charge. Village officials who charge or facilitate charges face criminal penalties.
  - _Unverified_: whether village cover letters (_surat pengantar_) count as population documents. Assume no fee field on population-related letters.

### Implications

- **Public site.** Never show NIK, KK number, date of birth or address on the public site. Public statistics must be aggregates. Suppress small cells (for example, under 3 people) so individuals cannot be identified in small _dusun_.
- **Audit log.** Make the audit log a core System feature (Art. 31, 32). Logging reads of individual records is the strict reading of "all processing".
- **Disability.** Do not store disability details, or store them only as an access-restricted field, since Adminduk Art. 84 protects them. Do not collect fingerprints or iris data.
- **Signatures.** Treat signature images (for example, the village head's signature on letters) as protected data. Restrict access to them and log their use.
- **Contracts.** The vendor needs a written processing agreement with each village (controller–processor terms, sub-processor approval, breach notice duty), plus a privacy notice for residents.
- **Imports and exports.** Spreadsheet imports and exports are processing. Log them. Exported files should be traceable (who, when, which filter).

## Open questions and unverified items

1. **PP 33/2026 (PDP implementing regulation).** Confirm its existence, number, effective date (reported as 16 January 2027) and DPIA/DPO/breach details against the official text.
2. **Permendagri 47/2016 status after PP 16/2026.** No replacement was found, but PP 16/2026 delegates village administration matters to new ministerial regulations. Watch jdih.kemendagri.go.id.
3. **Perki 1/2018 vs Perki 1/2021.** Confirm whether the village-specific standard is still in force.
4. **Permendagri 18/2018 (LKD).** Status after PP 16/2026.
5. **Kepmendagri 300.2.2-2138/2025 and -2430/2025.** Obtain the official Kemendagri copy of the code list.
6. **Retention schedule for village population records** (needed for PDP Art. 44).
7. **Dukcapil value lists** for marital status, religion, education and occupation, to align import columns with SIAK exports. This is likely in Permendagri 109/2019 or its successors and was not researched here.
8. **Fee status of village letters.** Whether any village service letter falls under the Adminduk no-fee rule.
