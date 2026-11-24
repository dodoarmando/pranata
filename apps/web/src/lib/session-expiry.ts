// Pure decision for the central 401 handler: only an expired session inside the
// admin area sends the user back to sign-in, and it returns to the same page.
export function expiryRedirect(
	code: string,
	currentPath: string,
): string | null {
	if (code !== "unauthenticated") return null;
	if (currentPath !== "/admin" && !currentPath.startsWith("/admin/")) {
		return null;
	}
	return `/masuk?ke=${encodeURIComponent(currentPath)}`;
}
