// Server-side base URL of the Pranata API.
const baseUrl = process.env.PRANATA_API_URL ?? 'http://127.0.0.1:8080';

/** Orval mutator: every generated call goes through here. */
export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${baseUrl}${path}`, init);
  const data: unknown = await response.json();
  return { data, status: response.status, headers: response.headers } as T;
}
