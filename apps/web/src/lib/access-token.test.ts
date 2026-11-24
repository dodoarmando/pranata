import { describe, expect, it } from "@rstest/core";
import { parseAccessToken } from "./access-token";

describe("parseAccessToken", () => {
	it("reads the token after the hash", () => {
		expect(parseAccessToken("#abc-DEF_123")).toBe("abc-DEF_123");
		expect(parseAccessToken("abc")).toBe("abc");
	});

	it("returns null when missing or malformed", () => {
		expect(parseAccessToken("")).toBeNull();
		expect(parseAccessToken("#")).toBeNull();
		expect(parseAccessToken("#a b")).toBeNull();
		expect(parseAccessToken(`#${"a".repeat(513)}`)).toBeNull();
	});
});
