import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "@rstest/core";
import {
	ACTION_LABELS,
	actionLabel,
	OBJECT_LABELS,
	objectLabel,
} from "./audit-labels";

const API_SRC = fileURLToPath(new URL("../../../api/src", import.meta.url));

function sources(dir: string): string[] {
	return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) return sources(path);
		return entry.name.endsWith(".rs") ? [path] : [];
	});
}

// Source text with each file's unit tests cut off, which may use made-up values.
function productionSources(): string[] {
	return sources(API_SRC).map(
		(file) => readFileSync(file, "utf8").split("#[cfg(test)]")[0] ?? "",
	);
}

function actions(): string[] {
	const audit = readFileSync(join(API_SRC, "audit.rs"), "utf8");
	const fn = /pub const fn as_str[\s\S]*?\n {4}\}\n/.exec(audit)?.[0] ?? "";
	return [...fn.matchAll(/=> "([a-z_]+)"/g)].map((m) => m[1] as string);
}

function objectTypes(): string[] {
	const found = new Set<string>();
	for (const text of productionSources()) {
		for (const m of text.matchAll(/\bobject_type: "([a-z_]+)"/g)) {
			found.add(m[1] as string);
		}
	}
	return [...found];
}

describe("audit labels", () => {
	it("labels every action the API can record", () => {
		const found = actions();
		// A sanity floor: the scan found the actions the API is known to record.
		for (const known of ["create", "update", "sign_in", "issue_access_link"]) {
			expect(found, `scan missed ${known}`).toContain(known);
		}
		expect(found.filter((a) => !Object.hasOwn(ACTION_LABELS, a))).toEqual([]);
	});

	it("labels every object type the API records", () => {
		const found = objectTypes();
		for (const known of ["user", "role", "village", "media_asset"]) {
			expect(found, `scan missed ${known}`).toContain(known);
		}
		expect(found.filter((t) => !Object.hasOwn(OBJECT_LABELS, t))).toEqual([]);
	});

	it("falls back to the raw key", () => {
		expect(actionLabel("deactivate")).toBe("Menonaktifkan");
		expect(actionLabel("brand_new")).toBe("brand_new");
		expect(objectLabel("media_asset")).toBe("Media");
		expect(objectLabel("constructor")).toBe("constructor");
	});
});
