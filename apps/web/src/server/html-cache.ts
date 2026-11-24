export type HtmlCacheOptions = {
	max?: number;
	ttlMs?: number;
	now?: () => number;
};

export type HtmlCache = {
	get(key: string): string | undefined;
	set(key: string, html: string): void;
	purgeAll(): void;
	size(): number;
};

type Entry = { html: string; expiresAt: number };

// In-memory LRU of rendered public pages. A Map keeps insertion order, so the
// first key is always the least recently used one.
export function createHtmlCache(opts: HtmlCacheOptions): HtmlCache {
	const max = opts.max ?? 500;
	const ttlMs = opts.ttlMs ?? 300_000;
	const now = opts.now ?? Date.now;
	const entries = new Map<string, Entry>();

	return {
		get(key) {
			const entry = entries.get(key);
			if (!entry) {
				return undefined;
			}
			if (entry.expiresAt <= now()) {
				entries.delete(key);
				return undefined;
			}
			entries.delete(key);
			entries.set(key, entry);
			return entry.html;
		},
		set(key, html) {
			entries.delete(key);
			entries.set(key, { html, expiresAt: now() + ttlMs });
			while (entries.size > max) {
				const oldest = entries.keys().next();
				if (oldest.done) {
					break;
				}
				entries.delete(oldest.value);
			}
		},
		purgeAll() {
			entries.clear();
		},
		size() {
			return entries.size;
		},
	};
}
