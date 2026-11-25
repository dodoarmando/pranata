import { describe, expect, it } from "@rstest/core";
import { step } from "./login-carousel";

describe("step", () => {
	it("wraps forward past the last slide", () => {
		expect(step(4, 1, 5)).toBe(0);
	});

	it("wraps backward past the first slide", () => {
		expect(step(0, -1, 5)).toBe(4);
	});

	it("moves within the range", () => {
		expect(step(2, 1, 5)).toBe(3);
	});
});
