import { describe, expect, it } from "@rstest/core";
import { formatAuditTime } from "./audit-time";

// 2026-11-25 04:00 UTC is 11.00 in Jakarta.
const NOW = new Date("2026-11-25T04:00:00Z");

describe("formatAuditTime", () => {
	it("says today and yesterday in the Village's time zone", () => {
		expect(formatAuditTime("2026-11-25T03:42:00Z", "Asia/Jakarta", NOW)).toBe(
			"Hari ini, 10.42",
		);
		expect(formatAuditTime("2026-11-24T01:30:00Z", "Asia/Jakarta", NOW)).toBe(
			"Kemarin, 08.30",
		);
	});

	it("writes older dates in full", () => {
		expect(formatAuditTime("2026-11-17T23:58:00Z", "Asia/Jakarta", NOW)).toBe(
			"18 Nov 2026, 06.58",
		);
	});

	it("uses the zone, not the viewer's clock, to pick the day", () => {
		// 17.30 UTC on the 24th is already the 25th, 00.30, in Jakarta.
		expect(formatAuditTime("2026-11-24T17:30:00Z", "Asia/Jakarta", NOW)).toBe(
			"Hari ini, 00.30",
		);
		expect(formatAuditTime("2026-11-24T17:30:00Z", "Asia/Jayapura", NOW)).toBe(
			"Hari ini, 02.30",
		);
	});
});
