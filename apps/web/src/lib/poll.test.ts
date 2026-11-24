import { describe, expect, it } from "@rstest/core";
import { pollDelay } from "./poll";

describe("pollDelay", () => {
	it("starts at 1 s and backs off to 3 s", () => {
		expect([0, 1, 2, 3, 5, 6, 40].map(pollDelay)).toEqual([
			1000, 1000, 1000, 2000, 2000, 3000, 3000,
		]);
	});
});
