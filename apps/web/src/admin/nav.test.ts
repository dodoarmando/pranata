import { describe, expect, it } from "@rstest/core";
import { visibleNav } from "./nav";

describe("visibleNav", () => {
	it("renders no group until a section page exists", () => {
		expect(visibleNav()).toEqual([]);
	});
});
