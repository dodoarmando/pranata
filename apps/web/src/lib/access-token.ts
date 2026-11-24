// The Access Link token lives in the URL fragment so it is never sent to a
// server. Returns null when the fragment holds no usable token.
export function parseAccessToken(hash: string): string | null {
	const token = (hash.startsWith("#") ? hash.slice(1) : hash).trim();
	if (token.length === 0 || token.length > 512 || /\s/.test(token)) {
		return null;
	}
	return token;
}
