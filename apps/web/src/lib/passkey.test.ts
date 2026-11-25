import { afterEach, describe, expect, it } from "@rstest/core";
import { isCancellation, passkeysSupported } from "./passkey";

const globals = globalThis as { window?: unknown };
const original = globals.window;

afterEach(() => {
	globals.window = original;
});

describe("passkeysSupported", () => {
	it("is false without a window", () => {
		globals.window = undefined;
		expect(passkeysSupported()).toBe(false);
	});

	it("is false without window.PublicKeyCredential", () => {
		globals.window = {};
		expect(passkeysSupported()).toBe(false);
	});

	it("is true when the browser has PublicKeyCredential", () => {
		globals.window = { PublicKeyCredential: class {} };
		expect(passkeysSupported()).toBe(true);
	});
});

describe("isCancellation", () => {
	it("is true for the browser's NotAllowedError", () => {
		const error = new Error("The operation was cancelled.");
		error.name = "NotAllowedError";
		expect(isCancellation(error)).toBe(true);
	});

	it("is false for anything else", () => {
		expect(isCancellation(new Error("boom"))).toBe(false);
		expect(isCancellation("NotAllowedError")).toBe(false);
	});
});
