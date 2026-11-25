import { ApiError } from "@/api/fetcher";

function isSudoRequired(error: unknown): boolean {
	return error instanceof ApiError && error.messageCode === "sudo_required";
}

// Runs a request; if the API asks for Sudo Mode, confirms the password and
// retries once. A second sudo_required is surfaced rather than looped on.
export async function withSudo<T>(
	run: () => Promise<T>,
	confirm: () => Promise<boolean>,
): Promise<T> {
	try {
		return await run();
	} catch (error) {
		if (!isSudoRequired(error)) throw error;
		if (!(await confirm())) throw error;
		return run();
	}
}
