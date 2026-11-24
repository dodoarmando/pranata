const ID = /^[A-Za-z0-9_-]{11}$/;
const HOSTS = new Set([
	"youtube.com",
	"www.youtube.com",
	"m.youtube.com",
	"music.youtube.com",
	"www.youtube-nocookie.com",
	"youtube-nocookie.com",
	"youtu.be",
]);

// Extracts the 11-character video id from a pasted YouTube link or bare id.
// Only the id is stored; the public site builds its own privacy-friendly embed.
export function parseYoutubeId(input: string): string | null {
	const text = input.trim();
	if (ID.test(text)) return text;
	let url: URL;
	try {
		url = new URL(/^[a-z]+:\/\//i.test(text) ? text : `https://${text}`);
	} catch {
		return null;
	}
	if (!HOSTS.has(url.hostname.toLowerCase())) return null;
	const segments = url.pathname.split("/").filter(Boolean);
	const candidate =
		url.hostname === "youtu.be"
			? segments[0]
			: segments[0] === "watch"
				? url.searchParams.get("v")
				: ["embed", "shorts", "live", "v"].includes(segments[0] ?? "")
					? segments[1]
					: undefined;
	return candidate && ID.test(candidate) ? candidate : null;
}
