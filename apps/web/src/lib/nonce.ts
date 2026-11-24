// The SSR render writes this token wherever a nonce is needed; the server entry
// swaps in a fresh nonce for every response, cached or not. Kept out of
// src/server so the client bundle never imports server code.
export const NONCE_PLACEHOLDER = "__PRANATA_NONCE__";
