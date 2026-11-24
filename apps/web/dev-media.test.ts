import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "@rstest/core";
import { mediaMiddleware, resolveMediaFile } from "./dev-media";

const ROOT = "/srv/media/public";

describe("resolveMediaFile", () => {
	test("maps a variant path under the root", () => {
		expect(resolveMediaFile(ROOT, "/c9/abc-480.webp")).toBe(
			"/srv/media/public/c9/abc-480.webp",
		);
	});

	test("ignores the query string", () => {
		expect(resolveMediaFile(ROOT, "/c9/abc-480.webp?v=1")).toBe(
			"/srv/media/public/c9/abc-480.webp",
		);
	});

	test("rejects paths that leave the root", () => {
		expect(resolveMediaFile(ROOT, "/../originals/c9/abc.jpg")).toBeNull();
		expect(resolveMediaFile(ROOT, "/%2e%2e/originals/c9/abc.jpg")).toBeNull();
		expect(resolveMediaFile(ROOT, "/c9/..%2f..%2fsecret")).toBeNull();
	});

	test("rejects the root itself and malformed escapes", () => {
		expect(resolveMediaFile(ROOT, "/")).toBeNull();
		expect(resolveMediaFile(ROOT, "/%E0%A4%A")).toBeNull();
	});
});

function fakeResponse() {
	const headers: Record<string, string> = {};
	const chunks: Buffer[] = [];
	let status = 0;
	let resolveDone: () => void;
	const done = new Promise<void>((r) => {
		resolveDone = r;
	});
	const res = {
		writeHead(code: number, h: Record<string, string>) {
			status = code;
			Object.assign(headers, h);
		},
		write(chunk: Buffer) {
			chunks.push(Buffer.from(chunk));
			return true;
		},
		end(chunk?: Buffer) {
			if (chunk) chunks.push(Buffer.from(chunk));
			resolveDone();
		},
		on() {
			return res;
		},
		once() {
			return res;
		},
		emit() {
			return true;
		},
	};
	return {
		res: res as unknown as ServerResponse,
		done,
		result: () => ({ status, headers, body: Buffer.concat(chunks) }),
	};
}

describe("mediaMiddleware", () => {
	const root = mkdtempSync(join(tmpdir(), "pranata-media-"));
	mkdirSync(join(root, "c9"));
	writeFileSync(join(root, "c9", "abc-480.webp"), "webp-bytes");

	test("serves an existing variant with its content type", async () => {
		const { res, done, result } = fakeResponse();
		let nextCalled = false;
		mediaMiddleware(root)(
			{ method: "GET", url: "/c9/abc-480.webp" } as IncomingMessage,
			res,
			() => {
				nextCalled = true;
			},
		);
		await done;
		expect(nextCalled).toBe(false);
		expect(result().status).toBe(200);
		expect(result().headers["Content-Type"]).toBe("image/webp");
		expect(result().body.toString()).toBe("webp-bytes");
	});

	test("answers 404 for a missing file", async () => {
		const { res, done, result } = fakeResponse();
		mediaMiddleware(root)(
			{ method: "GET", url: "/c9/missing.webp" } as IncomingMessage,
			res,
			() => {},
		);
		await done;
		expect(result().status).toBe(404);
	});

	test("passes other methods on", () => {
		let nextCalled = false;
		mediaMiddleware(root)(
			{ method: "POST", url: "/c9/abc-480.webp" } as IncomingMessage,
			fakeResponse().res,
			() => {
				nextCalled = true;
			},
		);
		expect(nextCalled).toBe(true);
	});
});
