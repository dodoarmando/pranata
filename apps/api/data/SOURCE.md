# Region code dataset

`region-codes.csv.gz` holds every village and kelurahan in Indonesia with the
names of its district, regency and province. Columns, in order:
`code,village,district,regency,province`. `code` is the dotted 10-digit village
code (`PP.KK.CC.VVVV`). Names are as published, so regencies keep their
`Kabupaten` or `Kota` prefix. The file has 83,762 data rows plus the header;
the `bundled_dataset_is_well_formed` test pins that count, so change both
together.

## Source

- Repository: <https://github.com/cahyadsn/wilayah>
- File: `db/wilayah.sql`
- Commit: `c2eff0881e3c75b8822f8c84ea9d23affc11aadb` (2026-11-10)
- SHA-256 of that file: `c4c3396d9380d4edee072af1d9dff83573b574d7cd00a6562cf82e200e954031`
- Retrieved: 2026-11-24
- Regulation: Kepmendagri No. 300.2.2-2430 Tahun 2025. The repository README
  states that `db/wilayah.sql` follows this regulation; the comment header of
  the SQL file still says 300.2.2-2138, which the README and change log
  indicate is stale.

## Licence

MIT, Copyright (c) 2017-2025 Cahya DSN. MIT is compatible with this project's
AGPL-3.0-or-later licence. The MIT notice must travel with copies of the data:

> Copyright (c) 2017-2025 Cahya DSN
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

Attribution: region data by Cahya DSN (cahyadsn/wilayah), derived from the
Kepmendagri code lists.

The other candidate, yonatanl/KODE-WILAYAH-KEPMENDAGRI-2025, was not used: its
last update predates the 2430 regulation and its licence is CC BY 4.0.

## Regenerating

```sh
curl -fL -o /tmp/wilayah.sql \
  https://raw.githubusercontent.com/cahyadsn/wilayah/c2eff0881e3c75b8822f8c84ea9d23affc11aadb/db/wilayah.sql
python3 -I tools/region-codes/build.py /tmp/wilayah.sql apps/api/data/region-codes.csv.gz
```

`tools/region-codes/build.py` keeps only 4-segment village codes, joins the
parent names, sorts by code and writes a deterministic gzip (mtime 0).
