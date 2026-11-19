import { getRequestHeader, getRequestIP, setResponseHeader } from '@tanstack/react-start/server';

// Server-side base URL of the Pranata API.
const baseUrl = process.env.PRANATA_API_URL ?? 'http://127.0.0.1:8080';

// The browser never reaches the API, so its session cookie, the headers the API's CSRF
// check reads and its user agent travel with each call, and the session cookie the API sets
// travels back.
const forwarded = ['cookie', 'origin', 'sec-fetch-site', 'user-agent'] as const;

/** Orval mutator: every generated call goes through here, always on the server. */
export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  for (const name of forwarded) {
    const value = getRequestHeader(name);
    if (value) headers.set(name, value);
  }
  // The reverse proxy in front sets X-Forwarded-For to the client alone; without one, the peer.
  const ip = getRequestIP({ xForwardedFor: true });
  if (ip) headers.set('x-forwarded-for', ip);
  const response = await fetch(`${baseUrl}${path}`, { ...init, headers });
  const setCookie = response.headers.get('set-cookie');
  if (setCookie) setResponseHeader('set-cookie', setCookie);
  // Refusals such as a malformed query come back as plain text, not JSON.
  const json = response.headers.get('content-type')?.startsWith('application/json');
  const data: unknown = json ? await response.json() : undefined;
  return { data, status: response.status, headers: response.headers } as T;
}
