// Development only: serve Media Asset variants at /media, the route Caddy
// serves on a real Installation, so `just dev` works without Caddy.
import { createReadStream, statSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import { extname, resolve, sep } from "node:path";

const TYPES: Record<string, string> = {
	".webp": "image/webp",
	".jpg": "image/jpeg",
	".jpeg": "image/jpeg",
	".png": "image/png",
	".pdf": "application/pdf",
};

// Returns the file under `root` that `urlPath` names, or null when the path is
// malformed or leaves `root`.
export function resolveMediaFile(root: string, urlPath: string): string | null {
	let path: string;
	try {
		path = decodeURIComponent(urlPath.split("?")[0] ?? "");
	} catch {
		return null;
	}
	const base = resolve(root);
	const file = resolve(base, `.${path}`);
	return file.startsWith(base + sep) ? file : null;
}

// A connect-style middleware for the path below the /media mount.
export function mediaMiddleware(root: string) {
	return (req: IncomingMessage, res: ServerResponse, next: () => void) => {
		if (req.method !== "GET" && req.method !== "HEAD") {
			next();
			return;
		}
		const file = resolveMediaFile(root, req.url ?? "/");
		const stat = file ? statSync(file, { throwIfNoEntry: false }) : undefined;
		if (!file || !stat?.isFile()) {
			res.writeHead(404, { "Content-Type": "text/plain" });
			res.end();
			return;
		}
		res.writeHead(200, {
			"Content-Type":
				TYPES[extname(file).toLowerCase()] ?? "application/octet-stream",
			"Content-Length": String(stat.size),
			"Cache-Control": "no-store",
		});
		if (req.method === "HEAD") {
			res.end();
			return;
		}
		createReadStream(file).pipe(res);
	};
}
