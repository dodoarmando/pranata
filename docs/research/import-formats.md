# Spreadsheet formats for population and APBDes data

Research for issue #5. Researched 2026-11-18.

Question: which spreadsheets do villages actually hold for population and
APBDes data, what do they look like (columns, merged headers, NIK typing, dates,
code tables), and what can calamine read?

## Answer in brief

- **Population.** Villages hold three kinds of file. All of them are defined or
  mirrored by OpenSID, the open-source village system that most Indonesian
  village software follows:
  1. **OpenSID's own import/export sheet** (`.xlsx`/`.xlsm`, sheet `Data
     Penduduk`, 44 snake_case columns, one row per resident). This is the
     cleanest format and the most widely held: every OpenSID village can export
     its residents to it.
  2. **The Dukcapil/SIAK flat export** (`.xls`, 37 Indonesian-titled columns,
     one row per resident). It uses labels where OpenSID uses codes, and it
     contains audit and officer columns.
  3. **"Buku Induk Penduduk" (BIP) printed-report layouts** (`.xls`, four
     variants). These have a title block, two-row merged headers, a row of
     column numbers, per-family group rows with the KK number and address packed
     into one string, and repeated header blocks.
- **Prodeskel** is a web application behind a login. No public import or export
  spreadsheet format was found. Don't target it.
- **APBDes.** Siskeudes (BPKP/Kemendagri) covers about 95% of villages. The only
  machine-readable exchange format in use is **not a spreadsheet**. It is a ZIP
  of CSV dumps of Siskeudes tables (`Ta_RAB.csv`, `Ta_RABRinci.csv`,
  `Ta_JurnalUmumRinci.csv`), and that is what OpenSID imports. Siskeudes report
  exports to Excel exist but have no public specification.
- **calamine 0.38.0** (2026-11-02, MSRV 1.88) reads `xls`, `xlsx`, `xlsm`,
  `xlsb` and `ods`, including merged-cell regions for xlsx and xls. It does
  **not** read CSV. XLSX can be streamed. XLS and ODS are loaded into memory
  whole when the file is opened.
- **NIK hazard.** Excel keeps only 15 significant digits. A 16-digit NIK or KK
  number that was ever stored as a _number_ has already lost its last digit.
  Accept NIK and KK numbers only from text cells.
- **Recommendation:** support the OpenSID sheet first, the SIAK flat export
  second, and the Siskeudes CSV ZIP for APBDes. Defer the BIP report layouts and
  Siskeudes Excel reports. Details are under [Recommendation](#recommendation).

## Sources

Primary sources, all read on 2026-11-18:

- OpenSID source, branch `umum`, <https://github.com/OpenSID/OpenSID>:
  - `app/Libraries/Import.php`: the native importer (column list, validation,
    normalisation).
  - `app/Libraries/BIP/{Bip,Siak,Bip2016,…}.php`: format detection and parsers
    for SIAK and BIP.
  - `donjo-app/controllers/Penduduk.php`: `ekspor()` (population export) and
    the template link.
  - `donjo-app/controllers/Keuangan_manual.php` and
    `donjo-app/helpers/database_helper.php` (`get_csv`): the Siskeudes import.
  - `donjo-app/models/migrations/struktur_tabel/*keuangan_ta_*`: OpenSID's
    mirror of the Siskeudes table columns.
  - Templates in `storage/app/template/impor/`: `format-impor-excel.xlsm`,
    `format-siak.xls`, `format-bip-2016.xls`, `format-bip-ektp.xls`. I
    downloaded these into a scratch directory outside the repo and only
    inspected them: zip/XML for the `.xlsm`, and a headless LibreOffice
    conversion for the `.xls` files, to see cell types and merged ranges. They
    contain sample personal data, so none of it is reproduced here.
- calamine: crates.io API (`/api/v1/crates/calamine`), `README.md` and
  `Changelog.md` at <https://github.com/tafia/calamine>, docs.rs pages for
  `Xlsx`, `Xls`, `XlsOptions` and `Data` at version 0.38.0, and Context7
  (`/tafia/calamine/v0.38.0`).
- Microsoft: "Excel specifications and limits"
  (<https://support.microsoft.com/en-us/office/excel-specifications-and-limits-1672b34d-7043-467e-8e27-269d656771c3>)
  and "Last digits are changed to zeros when you type long numbers in cells"
  (<https://support.microsoft.com/topic/1bf7b935-36e1-4985-842f-5dfa51f85fe7>).
- Prodeskel public site: <http://prodeskel.binapemdes.kemendagri.go.id/mpublik/>.
- Adoption figures come from news and secondary sources and are marked as such
  where they are used.

## 1. OpenSID native sheet (`format-impor-excel.xlsm`)

**Prevalence.** This is the default import format of OpenSID, and OpenSID's
"Ekspor" writes the same columns back out (`Penduduk::ekspor()` reuses
`Import::DAFTAR_KOLOM`). So it is both the blank template villages fill in by
hand and the file any OpenSID village can produce from its existing data.
OpenSID adoption: OpenDesa material from 2023 reports more than 6,500 villages
active daily. A 2025 journal article attributes "more than 24,000 villages have
used OpenSID, about 8,000 active daily" to OpenDesa, but that figure is
secondary.

**Workbook shape.**

- Sheet `Data Penduduk` holds the data. Sheet `Kode Data` holds the code tables,
  exposed as defined names (`agama`, `pekerjaan`, `hubungan_keluarga`, …) for
  in-cell dropdowns (data validation lists).
- Row 1 is the header and uses the machine keys below. The importer rejects any
  header it doesn't recognise. **Columns may appear in any order, and any subset
  may be present**, because the importer maps them by header name.
- Rows are read until a row whose column B is `###`. The template puts a green
  instruction block below that marker, so a reader must stop there.
- The template is a **macro-enabled `.xlsm`** containing `xl/vbaProject.bin`.
  The UI tells users to save as `.xlsx` before uploading, and the importer
  (OpenSpout's XLSX reader) accepts either.

**Columns (44), in template order:**

`alamat, dusun, rw, rt, nama, no_kk, nik, sex, tempatlahir, tanggallahir,
agama_id, pendidikan_kk_id, pendidikan_sedang_id, pekerjaan_id, status_kawin,
kk_level, warganegara_id, ayah_nik, nama_ayah, ibu_nik, nama_ibu,
golongan_darah_id, akta_lahir, dokumen_pasport, tanggal_akhir_paspor,
dokumen_kitas, akta_perkawinan, tanggalperkawinan, akta_perceraian,
tanggalperceraian, cacat_id, cara_kb_id, hamil, ktp_el, status_rekam,
alamat_sekarang, status_dasar, suku, tag_id_card, id_asuransi, no_asuransi,
lat, lng, ket`

**Code tables** (sheet `Kode Data`; the validation ranges come from
`Import::dataImportValid`):

| Column                 | Range                             | Examples                                                                 |
| ---------------------- | --------------------------------- | ------------------------------------------------------------------------ |
| `sex`                  | 1–2                               | LAKI-LAKI, PEREMPUAN (the importer also accepts `L`, `LK`, `P`, `PR`)    |
| `agama_id`             | 1–7                               | ISLAM … Kepercayaan Terhadap Tuhan YME                                   |
| `pendidikan_kk_id`     | 1–10                              | TIDAK / BELUM SEKOLAH … STRATA III                                       |
| `pendidikan_sedang_id` | 1–18                              | BELUM MASUK TK … SEDANG SLB C                                            |
| `pekerjaan_id`         | 1–89                              | BELUM/TIDAK BEKERJA, PETANI/PEKEBUN, …                                   |
| `status_kawin`         | 1–4                               | BELUM KAWIN, KAWIN, CERAI HIDUP, CERAI MATI (also `BK`, `K`, `CH`, `CM`) |
| `kk_level` (SHDK)      | 1–11                              | KEPALA KELUARGA, SUAMI, ISTRI, ANAK, … LAINNYA                           |
| `warganegara_id`       | 1–3                               | WNI, WNA, DUA KEWARGANEGARAAN                                            |
| `golongan_darah_id`    | 1–13                              | A, B, AB, O, A+ … O-, TIDAK TAHU (also `Tdk Th`)                         |
| `cacat_id`             | 1–7                               | CACAT FISIK … TIDAK CACAT                                                |
| `cara_kb_id`           | 1–8, 99 (template also lists 100) | Pil, IUD, Suntik, …                                                      |
| `hamil`, `ktp_el`      | 1–2                               |                                                                          |
| `status_rekam`         | 1–8                               | BELUM WAJIB, BELUM REKAM, SUDAH REKAM, CARD PRINTED, …                   |
| `status_dasar`         | 1,2,3,4,6,9                       | HIDUP, MATI, PINDAH, HILANG, PERGI, TIDAK VALID                          |
| `id_asuransi`          | 1–4, 99                           | TIDAK/BELUM PUNYA, BPJS PBI, …                                           |

These are the Dukcapil/SIAK reference lists, so the same labels appear in the
SIAK and BIP files.

**Quirks observed.**

- **Codes or labels.** Coded columns accept either the number or the label.
  Labels are matched case-insensitively, with spaces removed and `" / "`
  normalised to `/` (`Import::getKode`). Exported files use numbers by default
  and labels when the `$huruf` flag is set.
- **NIK and KK** are text cells in the template (`t="s"`). The importer strips
  every non-digit, because Dukcapil data "sometimes contains invisible
  characters" (comment in `getIsiBaris`). The rules are `nik` = 16 digits or
  `0` (no NIK yet, given a temporary NIK) and `no_kk` = 16 digits.
- **Dates.** In the template, dates are real Excel date serials with a date
  format (style 25). In OpenSID's **export**, every value is written as a
  string, so dates arrive as `YYYY-MM-DD` text. The importer accepts both by
  running them through `Carbon::parse`. Expect both typed dates and ISO text in
  the wild.
- **RW/RT** are zero-padded text (`"002"`, `"001"`), and `-` means "none". The
  importer strips a leading apostrophe.
- **Dusun** is normalised: upper-cased, `_` turned into a space, and a leading
  `DUSUN ` prefix removed.
- **Export escaping bug.** `Penduduk::bersihkanData` wraps any value that
  contains `"` in quotes and doubles the inner quotes, as if it were writing
  CSV, even though the output is xlsx. Strip that on import.
- **Empty code fields.** The export writes `''` for code fields stored as `0`,
  except `nik`/`no_kk`, where `0` is meaningful.

## 2. Dukcapil / SIAK flat export (`format-siak.xls`)

**Prevalence.** This is the shape of the resident listing that districts'
Disdukcapil offices hand to villages. SIAK itself is not accessible to
villages. OpenSID auto-detects it: cell A1 is `Nomor KK` and the 34th column is
`Petugas Registrasi` (`Bip::cariFormatBip`). The template is a legacy BIFF
`.xls` created on Mac Excel with code page 10000 (Mac Roman).

**Columns (37):** `Nomor KK, NIK, Nama Penduduk, Status, Tempat Lahir, Tanggal
Lahir, Kelamin, NIK Ayah, Ayah, NIK Ibu, Ibu, Status Kawin, SHDK, Agama,
Kecamatan, Desa/Kel, Alamat, RW, RT, Pendidikan, Pekerjaan, Deskripsi
Pekerjaan, Gol. Darah, Cacat, Cetak KTP, Ganti KTP, KTP Berakhir, Nomor Paspor,
Nomor Akta Lahir, Nomor Akta Kawin, Tanggal Kwn, Nomor Akta Cerai, Tgl Cerai,
Petugas Registrasi, Tgl Ubah, Petugas Entri, Tgl Entri`

**Quirks.**

- The file has one header row and no merged cells. Data starts at the first
  row whose first three cells are not all empty (`Siak::cariBarisPertama`).
- **Labels, not codes**, in title case (`Perempuan`, `Kawin`, `Kepala
  Keluarga`, `Tdk Th`). `Status` holds `Hidup` / `Tidak Valid` / …, and
  `Pekerjaan` needs `" / "` normalisation.
- NIK, KK and parent NIK columns are text in the sample. Dates are typed date
  cells.
- **RW/RT are numbers** (`8`, `1`), so zero-padding is lost. Normalise them to
  `"008"`/`"001"` (or whatever form the Pranata domain picks).
- Placeholders such as `-` appear in document-number columns. Treat them as
  empty.
- **Officer names and audit timestamps** (`Petugas Registrasi`, `Petugas
  Entri`, `Tgl Ubah`, `Tgl Entri`) are personal data about civil servants.
  Drop them on import.
- `Kecamatan`/`Desa/Kel` are constant for a single-village Installation. Use
  them only as a sanity check that the file belongs to this Village.

## 3. Buku Induk Penduduk report layouts (BIP 2012, 2016, 2016 regional variant, eKTP)

**Prevalence.** These are spreadsheet renderings of the printed resident
register (Buku Induk Penduduk) that some Disdukcapil offices distribute.
OpenSID ships four parsers for them, `Bip2012`, `Bip2016`, `Bip2016Luwutimur`
and `BipEktp`, and picks one by sniffing the title cells. That four exist shows
the layout varies by district and by year.

**Shape (BIP 2016 sample).**

- A title block (`BUKU INDUK PENDUDUK WNI`), then province, district,
  subdistrict and village as `label : value` cells, then the period date.
- A **two-row merged header**: for example, `Tempat dan Tanggal Lahir` spans
  `Tempat Lahir` and `Tgl/Bln/Th`, and `Nama Orang Tua Kandung` spans `Ibu` and
  `Ayah`. The sample has 12 to 19 merged ranges.
- A **column-number row** `(1) (2) …`. Excel parses `(1)` as accounting
  notation for a negative number, so the cells hold the numbers `-1, -2, …`.
- **Family group rows**: `No. KK : <16 digits>` in column A, and in a merged cell an
  address string such as `Alamat : …, Nama Dusun : …, RT/RW : 003/001`, which
  the parser takes apart by searching for the labels (`Bip2016` lines
  178–220). The eKTP variant puts the KK number, the head of family, the blank
  KK number, the print date and the address on separate label and value rows.
- The member rows follow: running number, name, NIK, place of birth, date of
  birth, sex (`Lk`/`Pr`), SHDK, religion, education, occupation, mother, father.
  The eKTP variant adds marital status (`BK`/`K`/`CM`), blood type, birth
  certificate number, `Wjb KTP`, `KTP-eL`, status and recording status.
- The header and column-number rows **repeat** for each page or family block.

**Quirks.** NIK cells are text with trailing spaces. Cells contain embedded
line breaks (`"SENT FOR\nCARD PRINTING"`, and birth certificate numbers split
across lines). Labels are abbreviated. Dates are typed date cells in the
samples.

**Verdict.** Parsing these files means scraping a report: it is fragile,
depends on the district, and the same data can be had from the SIAK flat export.
Defer.

## 4. Prodeskel

Prodeskel (Profil Desa dan Kelurahan, Kemendagri Bina Pemdes) is the
village-profile instrument under Permendagri 81/2015. Its public site offers
only menus and a login ("Masuk ke aplikasi untuk pengguna yang sudah
terdaftar"). Data is entered through web forms. No public spreadsheet template
or export specification was found, and the OpenSID tree has no Prodeskel
import, only a mention of an optional add-on module. Villages are not expected
to hold Prodeskel spreadsheets in a stable layout. **Not a v1 target.**
(Confidence: medium. A logged-in export may exist but could not be checked.)

## 5. APBDes: Siskeudes

**Prevalence.** Siskeudes, built by BPKP with Kemendagri, is the national
village finance application. Kemenkeu's Director of Village Funds put coverage
at **95.3% of villages** in August 2025, with about 3,000 villages not yet
reached because of connectivity (reported by Republika, 2025-08-08, a secondary
source). It runs as a desktop application with an online version alongside.

**What OpenSID imports** (`Keuangan_manual::proses_impor`):

- A **`.zip` of `.csv` files**, one per Siskeudes table. The form label is
  "Berkas Database Siskeudes", with the help text "format berkas .zip berisi
  data Siskeudes dalam format .csv". `get_csv` reads the first row as the
  header and **drops any row whose cell count differs from the header's**. It
  splits lines on `\r\n|\n|\r`, so a quoted field containing a newline breaks
  the row.
- Files used:
  - `Ta_RAB.csv`: OpenSID reads only `Tahun` from it. Columns: `Tahun, Kd_Desa,
    Kd_Keg, Kd_Rincian, Anggaran, AnggaranPAK, AnggaranStlhPAK, Kd_SubRinci`.
  - `Ta_RABRinci.csv`: the budget detail. Columns: `Tahun, Kd_Desa, Kd_Keg,
    Kd_Rincian, Kd_SubRinci, No_Urut, SumberDana, Uraian, Satuan, JmlSatuan,
    HrgSatuan, Anggaran, JmlSatuanPAK, HrgSatuanPAK, AnggaranStlhPAK,
    AnggaranPAK, Kode_SBU`. Budget = sum of `AnggaranStlhPAK` per
    `Kd_Rincian`.
  - `Ta_JurnalUmumRinci.csv`: the journal lines behind realisation. Columns:
    `Tahun, NoBukti, Kd_Keg, RincianSD, NoID, Kd_Desa, Akun, Kd_Rincian,
    Sumberdana, DK, Debet, Kredit`. OpenSID computes realisation as
    `Debet + Kredit` per `Kd_Rincian`. Treat that as OpenSID's simplification
    and don't copy it without checking against the `DK` (debit/credit) flag.
- **Account codes.** `Kd_Rincian` carries a trailing dot that OpenSID strips
  (`preg_replace('/\.$/', '', …)`) before matching it against the chart of
  accounts from Permendagri 20/2018 (4 = Pendapatan, 5 = Belanja, 6 =
  Pembiayaan). OpenSID's template levels have code lengths 1, 3, 5 and 8 or
  more, that is `4`, `4.1`, `4.1.1`, `4.1.1.01` (inferred from the
  `length(template_uuid)` queries in `Keuangan_manual`/`Keuangan`).
- **Amounts** are kept as strings in OpenSID's mirror tables (`string(…,
  100)`). Expect decimal text, and parse it as an exact decimal, never as a
  float.
- The CSV files come from the Siskeudes database tables. The exact export path
  (a built-in Siskeudes feature, a third-party tool, or the online version)
  isn't documented anywhere public that I could find. A Siskeudes 2.0 training
  outline lists "Ekspor Data Anggaran" and "Ekspor Data Penatausahaan" menus but
  doesn't describe their output.

**Siskeudes report exports to Excel** (for example Laporan Realisasi APBDes)
are report-shaped: account-code hierarchy rows with Anggaran / Realisasi /
Lebih (Kurang) columns, following the Permendagri 20/2018 annexes. No primary
specification of the Excel layout was found, and they would need the same
report-scraping as the BIP files. **Confidence: low.** Get a real sample from
the product owner before committing to this path.

## 6. calamine

| Item           | Finding                                                                                                                                                                                                                                                                                  | Source                      |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| Latest stable  | **0.38.0**, published 2026-11-02, not yanked                                                                                                                                                                                                                                             | crates.io API               |
| MSRV           | 1.88 for 0.38.0 (crates.io `rust_version`). `master` has since moved to 1.89                                                                                                                                                                                                             | crates.io API, `Cargo.toml` |
| Formats        | `xls`, `xlsx`, `xlsm`, `xlsb`, `xla`, `xlam`, `ods`. Read-only                                                                                                                                                                                                                           | README                      |
| CSV            | **Not supported.** The Siskeudes ZIP needs the `zip` and `csv` crates (an addition to the chosen stack)                                                                                                                                                                                  | README (format list)        |
| Unknown type   | `open_workbook_auto(path)` picks the reader from the file extension                                                                                                                                                                                                                      | README, docs.rs             |
| Cell values    | `Data::{Int, Float, String, Bool, DateTime(ExcelDateTime), DateTimeIso, DurationIso, Error, Empty}`                                                                                                                                                                                      | docs.rs `Data`              |
| Dates          | Number cells with a date format come back as `DateTime(ExcelDateTime)`. `as_date`/`as_datetime` need the `chrono` feature. `has_1904_epoch()` exists on Xls, Xlsx and Xlsb. Dates typed as text stay `String`                                                                            | docs.rs, Changelog          |
| Merged cells   | `merge_cells_by_sheet_name` / `merge_cells_by_sheet_id` on `Xlsx` and `Xls`, returning `Vec<Dimensions>`. The older XLSX methods (`worksheet_merge_cells`) were deprecated in a recent unification. ODS "covered cells" are handled. Only the top-left cell of a merge carries the value | docs.rs, Changelog          |
| Header row     | `with_header_row(HeaderRow::Row(n))` or the first non-empty row                                                                                                                                                                                                                          | README                      |
| Large sheets   | XLSX/XLSB load sheets lazily, and `Xlsx::worksheet_cells_reader` iterates cells one at a time. **XLS and ODS are fully parsed and held in memory when opened**                                                                                                                           | README, docs.rs             |
| Styles         | `worksheet_style()` (new in 0.38) returns number formats and other styles, **XLSX only**                                                                                                                                                                                                 | Changelog 0.38.0            |
| Encodings      | `XlsOptions { force_codepage, header_row }` covers legacy `.xls` with a missing or wrong code page (the OpenSID samples use Mac Roman, 10000)                                                                                                                                            | docs.rs `XlsOptions`        |
| Formulas / VBA | Values come from the cached results. `worksheet_formula` and `vba_project` are available. Macros are never run                                                                                                                                                                           | README                      |
| Whitespace     | XLSX shared and inline strings are trimmed of leading and trailing ASCII whitespace unless `xml:space="preserve"` is set (a recent change), so trailing spaces on NIKs may or may not survive. Trim anyway                                                                               | Changelog                   |
| Hardening      | 0.38.0 fixed a panic and unbounded allocation in the CFB (XLS) reader on malformed files. Pin ≥ 0.38 and still cap upload size and parse time                                                                                                                                            | Changelog 0.38.0            |

**Size check.** A village has a few thousand residents, far below Excel's
limit of 1,048,576 rows by 16,384 columns (Microsoft). Even the in-memory XLS
path is fine at that size. The real risks are malformed or hostile files, which
need an upload size cap, a timeout and parsing off the request thread, not
volume.

**NIK and KK as numbers.** Excel's numeric precision is 15 digits. For 16-digit
values, "any numbers past the 15th digit are rounded down to zero" (Microsoft).
A NIK cell that calamine returns as `Float`/`Int` has therefore _already_ lost
its last digit, and nothing can recover it. Import rules:

- Accept NIK, KK and parent NIK only from `String` cells, after stripping
  non-digits as OpenSID does.
- Reject numeric NIK/KK cells with a row error telling the user to format the
  column as Text, or prefix the values with `'`, and re-enter or re-export them.
  Scientific-notation display (`1.23457E+15`) is just how Excel shows such a
  cell. The damage happened when the value was stored.
- Keep accepting `0` as "NIK not yet issued", following OpenSID's convention,
  if the domain adopts it.

## Recommendation

1. **v1 population import: the OpenSID sheet (`.xlsx`/`.xlsm`, sheet `Data
   Penduduk`).** Use the same snake_case header keys, allow any column order or
   subset, stop at `###`, and accept a code or a label for each coded column.
   Pranata's own downloadable template should _be_ this format, with a code
   sheet and text-formatted NIK/KK/RW/RT columns. That way one parser covers
   villages migrating from OpenSID, which addresses the map's "Migration from
   existing village systems" item, as well as new data entry. Read it through
   `open_workbook_auto` so that `.xls` and `.ods` re-saves also work.
2. **v1.x population import: the SIAK flat export (`.xls`).** It has one header
   row and maps cleanly onto the same model through a label-to-code table that
   the OpenSID format needs anyway. Detect it by its header (`Nomor KK` …
   `Petugas Registrasi`) and drop the officer and audit columns.
3. **APBDes: the Siskeudes CSV ZIP** (`Ta_RAB`, `Ta_RABRinci`,
   `Ta_JurnalUmumRinci`). It is a typed, stable table dump keyed by
   Permendagri 20/2018 account codes. It needs `zip` + `csv`, not calamine. Use
   a real CSV parser, not line splitting, and decimal amounts. Keep manual
   entry as the fallback. This answers the map's "APBDes data source" question
   in favour of import, subject to getting one real export to confirm the
   column set.
4. **Defer:** the BIP report layouts (four district variants, merged headers,
   packed address strings) and Siskeudes Excel reports (no public
   specification). Revisit only if a pilot village has nothing else.
5. **Not a target:** Prodeskel.

Applies to all importers: reject numeric NIK/KK cells, normalise RW/RT to one
padded text form, parse dates from both `DateTime` cells and `YYYY-MM-DD` /
`DD-MM-YYYY` text, trim and NFC-normalise strings, and report errors per row
the way OpenSID does (row number + reason) instead of failing the whole file.

## Open questions and uncertainty

- Siskeudes: which built-in menu or external tool produces the CSV ZIP, and
  whether Siskeudes online offers a different export. Needs a real sample.
- Whether districts still distribute BIP-style files or now only send the flat
  SIAK listing. Prevalence is inferred from OpenSID shipping parsers for them,
  not measured.
- OpenSID adoption numbers are secondary (a journal article citing OpenDesa).
- calamine's behaviour on these specific files was not run hands-on, because I
  didn't build or execute downloaded code for this research. Write a fixture
  test with synthetic (non-personal) copies of each layout before relying on
  it.
