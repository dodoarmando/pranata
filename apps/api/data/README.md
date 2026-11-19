# National region code list

`regions.tsv` is the national code list `pranata village set-code` picks the Village Code from. It is embedded in the `pranata` binary.

- **Content**: every province, regency/city, district and village/kelurahan code with its name, per Kepmendagri 300.2.2-2138/2025 as amended by Kepmendagri 300.2.2-2430/2025. One `code<TAB>name` row each, with dotted codes (`11.01.01.2001`).
- **Source**: [`cahyadsn/wilayah`](https://github.com/cahyadsn/wilayah), `db/wilayah.sql` at commit `d68e8d5516f969d1905d0b2940f20034becb0db7` (2026-02-13).
- **Licence**: MIT, see `regions.LICENSE`. MIT is compatible with AGPL-3.0-only.
- **Real names**: this official public list is the one place the repo holds real region names; tests and examples refer to codes only.
- **Verified** against the decree totals: 38 provinces, 416 regencies + 98 cities, 7,285 districts, 8,496 kelurahan and 75,266 villages (75,252 desa + 14 desa adat).

Regenerate from a newer `wilayah.sql` (CRLF stripped, SQL quotes unescaped, trailing spaces trimmed):

```sh
tr -d '\r' < wilayah.sql | grep -E "^\('" \
  | sed -E "s/^\('([0-9.]+)','(.*)'\)[,;]$/\1\t\2/; s/''/'/g; s/ +$//" > regions.tsv
```

# Common and breached passwords

`common-passwords.txt` is the list a new password is checked against. It is embedded in the `pranata` binary.

- **Content**: the entries of 12–128 characters, lowercased (ASCII) and deduplicated, from the top million breached passwords plus the Indonesian top 150; shorter ones can never pass the length rule. One per line, sorted.
- **Source**: [`danielmiessler/SecLists`](https://github.com/danielmiessler/SecLists) at commit `abe37a24be7dd50d676840c9f63dca87de5117f8`: `Passwords/Common-Credentials/Pwdb_top-1000000.txt` and `Passwords/Common-Credentials/Language-Specific/Indonesian_Pwdb_common-password-list-top-150.txt`.
- **Licence**: MIT, see `common-passwords.LICENSE`. MIT is compatible with AGPL-3.0-only.

Regenerate:

```sh
cat Pwdb_top-1000000.txt Indonesian_Pwdb_common-password-list-top-150.txt | tr -d '\r' \
  | awk 'length($0) >= 12 && length($0) <= 128' | LC_ALL=C tr 'A-Z' 'a-z' | LC_ALL=C sort -u > common-passwords.txt
```
