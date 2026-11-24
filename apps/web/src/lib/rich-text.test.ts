import { describe, expect, it } from "@rstest/core";
import { asDoc, isEmptyDoc, sanitizeDoc } from "./rich-text";

const ID = "0192f2c4-7a2e-7c2b-8f3a-2f6f6c1d9a10";

describe("sanitizeDoc", () => {
	it("keeps only allowlisted nodes, marks and attrs", () => {
		const doc = sanitizeDoc({
			type: "doc",
			content: [
				{ type: "heading", attrs: { level: 1 }, content: [] },
				{
					type: "paragraph",
					attrs: { class: "x" },
					content: [
						{
							type: "text",
							text: "hi",
							marks: [
								{ type: "underline" },
								{ type: "bold" },
								{
									type: "link",
									attrs: { href: "https://a.test", target: "_blank", rel: "x" },
								},
							],
						},
					],
				},
				{ type: "codeBlock", content: [{ type: "text", text: "kept" }] },
				{ type: "image", attrs: { assetId: ID, alt: "", src: "/x" } },
				{ type: "image", attrs: { assetId: "nope" } },
				{ type: "youtube", attrs: { videoId: "dQw4w9WgXcQ", src: "x" } },
				{ type: "youtube", attrs: { videoId: "short" } },
				{
					type: "table",
					content: [
						{
							type: "tableRow",
							content: [
								{
									type: "tableCell",
									attrs: { colspan: 1 },
									content: [{ type: "paragraph", content: [] }],
								},
							],
						},
					],
				},
			],
		});
		expect(doc).toEqual({
			type: "doc",
			content: [
				{ type: "heading", attrs: { level: 2 }, content: [] },
				{
					type: "paragraph",
					content: [
						{
							type: "text",
							text: "hi",
							marks: [
								{ type: "link", attrs: { href: "https://a.test" } },
								{ type: "bold" },
							],
						},
					],
				},
				{ type: "text", text: "kept" },
				{ type: "image", attrs: { assetId: ID } },
				{ type: "youtube", attrs: { videoId: "dQw4w9WgXcQ" } },
				{
					type: "table",
					content: [
						{
							type: "tableRow",
							content: [
								{
									type: "tableCell",
									content: [{ type: "paragraph", content: [] }],
								},
							],
						},
					],
				},
			],
		});
	});

	it("drops links that are not http, https, mailto or tel", () => {
		const doc = sanitizeDoc({
			type: "doc",
			content: [
				{
					type: "paragraph",
					content: [
						{
							type: "text",
							text: "x",
							marks: [{ type: "link", attrs: { href: "javascript:alert(1)" } }],
						},
					],
				},
			],
		});
		expect(doc.content[0]?.content?.[0]).toEqual({ type: "text", text: "x" });
	});
});

describe("isEmptyDoc and asDoc", () => {
	it("treats blank paragraphs as empty", () => {
		expect(
			isEmptyDoc({
				type: "doc",
				content: [{ type: "paragraph", content: [] }],
			}),
		).toBe(true);
		expect(
			isEmptyDoc({
				type: "doc",
				content: [
					{ type: "paragraph", content: [{ type: "text", text: "a" }] },
				],
			}),
		).toBe(false);
	});

	it("rejects non-documents", () => {
		expect(asDoc(null)).toBeNull();
		expect(asDoc({ type: "paragraph" })).toBeNull();
		expect(asDoc({ type: "doc" })).toEqual({ type: "doc", content: [] });
	});
});
