import { describe, expect, it } from "@rstest/core";
import { can, PERMISSION_GROUPS, PERMISSION_LABELS } from "./permissions";

describe("PERMISSION_GROUPS", () => {
	it("covers every labelled permission exactly once", () => {
		const grouped = PERMISSION_GROUPS.flatMap((g) => g.permissions).sort();
		expect(grouped).toEqual(Object.keys(PERMISSION_LABELS).sort());
	});
});

describe("can", () => {
	it("is false without a session", () => {
		expect(can(null, "audit.read")).toBe(false);
	});
});
