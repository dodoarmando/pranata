import { describe, expect, it } from "@rstest/core";
import { createHtmlCache } from "./html-cache";

describe("html cache", () => {
	it("evicts least recently used", () => {
		const cache = createHtmlCache({ max: 2, ttlMs: 1000 });
		cache.set("/a", "A");
		cache.set("/b", "B");
		// Reading /a makes /b the least recently used entry.
		expect(cache.get("/a")).toBe("A");
		cache.set("/c", "C");
		expect(cache.get("/b")).toBeUndefined();
		expect(cache.get("/a")).toBe("A");
		expect(cache.get("/c")).toBe("C");
		expect(cache.size()).toBe(2);
	});

	it("expires after ttl", () => {
		let now = 1_000;
		const cache = createHtmlCache({ max: 10, ttlMs: 500, now: () => now });
		cache.set("/a", "A");
		now += 499;
		expect(cache.get("/a")).toBe("A");
		now += 2;
		expect(cache.get("/a")).toBeUndefined();
		expect(cache.size()).toBe(0);
	});

	it("purgeAll empties", () => {
		const cache = createHtmlCache({ max: 10, ttlMs: 1000 });
		cache.set("/a", "A");
		cache.set("/b", "B");
		cache.purgeAll();
		expect(cache.size()).toBe(0);
		expect(cache.get("/a")).toBeUndefined();
	});

	it("uses the documented defaults", () => {
		const cache = createHtmlCache({});
		cache.set("/a", "A");
		expect(cache.get("/a")).toBe("A");
	});
});
