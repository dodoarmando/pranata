export const POLL_TIMEOUT_MS = 2 * 60 * 1000;

// 1 s for the first few polls, then backing off to 3 s.
export function pollDelay(attempt: number): number {
	if (attempt < 3) return 1000;
	if (attempt < 6) return 2000;
	return 3000;
}
