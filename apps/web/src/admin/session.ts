import { ApiError } from "@/api/fetcher";
import { getMe } from "@/api/generated/client";
import type { SessionInfo } from "@/api/generated/model";
import { setCsrfToken } from "@/lib/csrf";

// Resolves the signed-in user, or null when there is no valid session.
// Also refreshes the CSRF token the fetcher sends on unsafe requests.
export async function loadSession(): Promise<SessionInfo | null> {
	try {
		const session = await getMe();
		setCsrfToken(session.csrf_token);
		return session;
	} catch (error) {
		if (error instanceof ApiError && error.status === 401) {
			setCsrfToken(null);
			return null;
		}
		throw error;
	}
}

// Two-letter initials for the avatar in the sidebar footer.
export function initials(displayName: string): string {
	const parts = displayName.trim().split(/\s+/).filter(Boolean);
	const letters = parts.slice(0, 2).map((part) => part.charAt(0));
	return letters.join("").toUpperCase() || "?";
}
