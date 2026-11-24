import { getCsrfToken } from "@/lib/csrf";
import type { Problem } from "./generated/model";

// Thrown for every non-2xx response. Pages call `messageFor(err.code)`.
export class ApiError extends Error {
	readonly status: number;
	readonly code: string;
	readonly problem: Problem | null;

	constructor(status: number, problem: Problem | null) {
		super(problem?.title ?? `HTTP ${status}`);
		this.name = "ApiError";
		this.status = status;
		this.code = problem?.code ?? "internal_error";
		this.problem = problem;
	}
}

// Error type for the generated react-query hooks (Orval picks this export up).
export type ErrorType<_E> = ApiError;
export type BodyType<T> = T;

// On the server there is no page origin to resolve /api against.
function serverApiOrigin(): string {
	return process.env.PRANATA_API_INTERNAL_URL || "http://127.0.0.1:8080";
}

function resolveUrl(url: string): string {
	return typeof window === "undefined" ? `${serverApiOrigin()}${url}` : url;
}

async function readProblem(response: Response): Promise<Problem | null> {
	try {
		return (await response.json()) as Problem;
	} catch {
		return null;
	}
}

// Fetch mutator used by every generated hook (see orval.config.ts).
export async function apiFetch<T>(
	url: string,
	options: RequestInit = {},
): Promise<T> {
	const headers = new Headers(options.headers);
	const method = (options.method ?? "GET").toUpperCase();
	const csrf = getCsrfToken();
	// The API requires the token on every non-GET admin request and on logout;
	// sending it on any unsafe request is harmless and keeps this rule simple.
	if (csrf && method !== "GET" && method !== "HEAD") {
		headers.set("x-csrf-token", csrf);
	}

	const response = await fetch(resolveUrl(url), {
		...options,
		headers,
		credentials: "include",
	});

	if (!response.ok) {
		throw new ApiError(response.status, await readProblem(response));
	}
	if (response.status === 204) {
		return undefined as T;
	}
	const type = response.headers.get("content-type") ?? "";
	// `updated_at` and other strings pass through untouched: no Date parsing.
	return (
		type.includes("json") ? await response.json() : await response.blob()
	) as T;
}
