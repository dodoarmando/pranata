import { describe, expect, it } from "@rstest/core";
import { parseYoutubeId } from "./youtube-id";

describe("parseYoutubeId", () => {
	it.each([
		"https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10s",
		"https://youtu.be/dQw4w9WgXcQ?si=abc",
		"https://www.youtube.com/embed/dQw4w9WgXcQ",
		"https://m.youtube.com/shorts/dQw4w9WgXcQ",
		"youtube.com/watch?v=dQw4w9WgXcQ",
		"dQw4w9WgXcQ",
	])("reads the id from %s", (input) => {
		expect(parseYoutubeId(input)).toBe("dQw4w9WgXcQ");
	});

	it.each([
		"",
		"https://example.com/watch?v=dQw4w9WgXcQ",
		"https://www.youtube.com/watch?v=short",
		"not a link",
	])("rejects %s", (input) => {
		expect(parseYoutubeId(input)).toBeNull();
	});
});
