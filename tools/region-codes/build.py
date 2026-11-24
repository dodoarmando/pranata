#!/usr/bin/env python3
"""Build apps/api/data/region-codes.csv.gz from cahyadsn/wilayah db/wilayah.sql.

Usage: build.py <path-to-wilayah.sql> <output.csv.gz>

Keeps only villages (4-segment codes, e.g. 14.05.03.1001) and joins each with
its district, regency and province names. Output is deterministic.
"""
import csv
import gzip
import io
import re
import sys

ROW = re.compile(r"^\('([0-9.]+)','((?:[^']|'')*)'\)[,;]?\s*$")


def main(source: str, target: str) -> None:
    names: dict[str, str] = {}
    with open(source, encoding="utf-8") as handle:
        for line in handle:
            match = ROW.match(line)
            if match:
                if match.group(1) in names:
                    raise SystemExit(f"duplicate code {match.group(1)}")
                names[match.group(1)] = match.group(2).replace("''", "'")

    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(["code", "village", "district", "regency", "province"])
    count = 0
    for code in sorted(names):
        parts = code.split(".")
        if len(parts) != 4 or [len(p) for p in parts] != [2, 2, 2, 4]:
            continue
        province = names.get(parts[0])
        regency = names.get(".".join(parts[:2]))
        district = names.get(".".join(parts[:3]))
        if None in (province, regency, district):
            raise SystemExit(f"orphan village code {code}")
        writer.writerow([code, names[code], district, regency, province])
        count += 1

    with open(target, "wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=9) as gz:
            gz.write(out.getvalue().encode("utf-8"))
    print(f"wrote {count} villages to {target}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    main(sys.argv[1], sys.argv[2])
