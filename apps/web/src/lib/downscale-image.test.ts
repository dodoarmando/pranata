import { describe, expect, it } from "@rstest/core";
import { downscaleImage, fitWithin, jpegName } from "./downscale-image";

describe("fitWithin", () => {
	it("keeps images that already fit", () => {
		expect(fitWithin(1200, 800, 2560)).toEqual({ width: 1200, height: 800 });
	});

	it("scales the longest edge down and keeps the ratio", () => {
		expect(fitWithin(4000, 3000, 2000)).toEqual({ width: 2000, height: 1500 });
		expect(fitWithin(3000, 6000, 2000)).toEqual({ width: 1000, height: 2000 });
	});
});

describe("jpegName", () => {
	it("swaps the extension", () => {
		expect(jpegName("IMG_0001.HEIC")).toBe("IMG_0001.jpg");
		expect(jpegName("photo")).toBe("photo.jpg");
		expect(jpegName("a.b.png")).toBe("a.b.jpg");
	});
});

describe("downscaleImage", () => {
	it("passes non-images through untouched", async () => {
		const file = new File(["%PDF"], "a.pdf", { type: "application/pdf" });
		expect(await downscaleImage(file)).toBe(file);
	});
});
