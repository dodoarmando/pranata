# Passkeys, sessions and password hashing in Rust

Research for issue #6. Answers: which libraries and settings Pranata's axum backend should use for staff passkeys, server-side sessions, Argon2id password hashing, "remember me", and CSRF protection when TanStack Start SSR and the axum API share one origin behind Caddy.

Versions checked against the crates.io and npm registries on 2026-11-18. Re-check them when implementing.

## Summary

| Topic            | Recommendation                                                                                                                                                                                                                                                                                  |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Passkeys         | `webauthn-rs` 0.6.2, default features only. Use the `passkey` flow, username-first (email, then passkey). No attestation and no conditional UI. Keep ceremony state in memory on the server.                                                                                                    |
| Sessions         | A small hand-rolled `sessions` table in PostgreSQL. The cookie holds a random 256-bit token and the database stores only its SHA-256 hash. Idle and absolute expiry are enforced on the server. `tower-sessions` is a viable fallback.                                                          |
| Cookie           | `__Host-session`, `Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`, no `Domain`. Without "remember me", no `Max-Age` (the cookie dies with the browser). With "remember me", `Max-Age` equals the absolute lifetime.                                                                              |
| Password hashing | `argon2` 0.6.0, Argon2id with m=19456 KiB, t=2, p=1 (the crate defaults and an OWASP minimum), pinned explicitly in code. Hash inside `spawn_blocking`. Rehash on login when the parameters change. No pepper.                                                                                  |
| Remember me      | A longer server-side lifetime for the session on that browser, not a skipped factor. Suggested defaults: 30 min idle and 12 h absolute without it; 7 days idle and 30 days absolute with it. Sensitive actions require recent authentication ("sudo mode"). The final values belong to #9.      |
| CSRF             | No CSRF tokens. An axum middleware rejects unsafe methods unless `Sec-Fetch-Site: same-origin`, or, when that header is missing, `Origin` exactly matches the Installation's origin. JSON-only bodies. TanStack Start's `createCsrfMiddleware` guards server functions. SSR loaders only `GET`. |

## Crate and package versions

| Package                     | Version              | Notes                                                                           | Source        |
| --------------------------- | -------------------- | ------------------------------------------------------------------------------- | ------------- |
| `webauthn-rs`               | 0.6.2 (2026-10-31)   | MSRV 1.89; default feature `attestation`                                        | crates.io API |
| `argon2`                    | 0.6.0 (2026-08-27)   | MSRV 1.85; default features `alloc`, `getrandom`, `password-hash`               | crates.io API |
| `password-hash`             | 0.6.1                | Re-exported by `argon2`                                                         | crates.io API |
| `tower-sessions`            | 0.15.0               | Fallback option only                                                            | crates.io API |
| `tower-sessions-sqlx-store` | 0.16.0 (2026-11-11)  | Depends on `sqlx ^0.9.0`; feature `postgres`                                    | crates.io API |
| `sqlx`                      | 0.9.1                |                                                                                 | crates.io API |
| `axum`                      | 0.8.9                |                                                                                 | crates.io API |
| `axum-extra`                | 0.12.6               | Has a `cookie` feature for reading and writing cookies                          | crates.io API |
| `getrandom`                 | 0.4.3                | For session tokens                                                              | crates.io API |
| `sha2`                      | 0.11.1               | For hashing session tokens at rest                                              | crates.io API |
| `@tanstack/react-start`     | 1.169.5 (2026-11-08) | Exports `createCsrfMiddleware` (checked in the published `dist/esm/index.d.ts`) | npm registry  |

## 1. Passkeys: webauthn-rs

### Maturity

- The project is maintained by the Kanidm team. The repository was active in November 2026, and the README states: "This library has passed a security audit performed by SUSE product security." ([README](https://github.com/kanidm/webauthn-rs))
- It is the only mature WebAuthn relying-party library for Rust. No hand-rolled WebAuthn.

### What "passkey" means in this crate

- The crate defines a passkey as "any possible authenticator": a security key, a TPM, Touch ID, or a synced credential. ([crate docs](https://docs.rs/webauthn-rs/0.6.2/webauthn_rs/))
- `start_passkey_registration` calls `require_resident_key(false)` and requires user verification through `credProtect` (`UserVerificationRequired`). (source `src/lib.rs` in the 0.6.2 crate)
- The README explains that the crate always _discourages_ resident keys, because they can fill and "brick" hardware keys. "Synchronised credential managers that always create resident keys (like iCloud Keychain) still work in this mode without issue." ([README](https://github.com/kanidm/webauthn-rs))

### Discoverable (usernameless) login

- Usernameless login is only available behind the `conditional-ui` feature (`start_discoverable_authentication` and `finish_discoverable_authentication_v2`). The docs say: "the platform and browser user experience is not good enough to justify enabling these flows at present", and of conditional UI: "We don't recommend conditional UI as a result." ([crate docs](https://docs.rs/webauthn-rs/0.6.2/webauthn_rs/))
- **Recommendation:** use the username-first flow. The prototype login already asks for an email first: the server looks up that staff member's passkeys and calls `start_passkey_authentication(&passkeys)`. Leave `conditional-ui` off.

### Attestation

- Attested flows (`start_attested_passkey_registration`) restrict registration to a known list of hardware. The crate notes that synced passkeys from Apple iCloud Keychain and Google Password Manager "can never be attested". (source `src/lib.rs`, `attestation` impl block)
- **Recommendation:** don't use attestation. Village staff use their own phones, which typically hold synced passkeys. The `attestation` feature is on by default and costs nothing when its methods are unused. Disabling it (`default-features = false`) is optional.
- Never enable `danger-user-presence-only-security-keys` ("If in doubt, do not enable this feature"). Never enable `workaround-google-passkey-specific-issues`: its flow "must NEVER be triggered on any other class of device or browser" than Android with GMS Core. ([crate docs](https://docs.rs/webauthn-rs/0.6.2/webauthn_rs/))

### Ceremony state

- The crate requires the registration and authentication state to be stored server-side ("YOU MUST STORE THE [PasskeyRegistration] VALUE SERVER SIDE"). Storing it in a cookie allows replay "bypassing pretty much all of the security guarantees of webauthn". By default these state types are not serialisable. ([crate docs](https://docs.rs/webauthn-rs/0.6.2/webauthn_rs/))
- **Recommendation:** because one Installation runs one API process (ADR-0001), keep the state in an in-memory map in `AppState`. Key it by a random ceremony ID with a short TTL (about 5 minutes) and remove the entry on first use. This needs no feature flag and no table. A restart only interrupts in-flight ceremonies, and the user retries.
  - `shortcut: in-memory ceremony state assumes a single API process; if the API ever scales out, enable danger-allow-state-serialisation and move it to a Postgres table with expiry.`

### Credential storage

- `Passkey` is `Serialize`/`Deserialize`, and the docs say it "can be safely serialised and deserialised from a database". (source `src/interface.rs`)
- Suggested table `passkeys`:
  - `id` (UUIDv7)
  - `user_id`
  - `credential_id bytea unique`
  - `passkey jsonb`
  - `name`
  - `created_at`
  - `last_used_at`
- After each login, call `passkey.update_credential(&auth_result)` and write the row back only when it returns `Some(true)`. The docs note that most synced passkeys never change, because they have no counter. (source `src/interface.rs`)
- Pass the staff member's existing credential IDs as `exclude_credentials` when they register another passkey.
- **RP ID binding:** `WebauthnBuilder::new(rp_id, &rp_origin)` binds every credential to the Installation's domain. If a village changes its domain, all its passkeys stop working. Passwords stay as the recovery path. Covering this in the operator runbook is a decision for #9.

## 2. Sessions

### Requirements from the sources

- Session IDs: "at least 64 bits of entropy", generated by a CSPRNG of "at least 128 bits". ([OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html))
- Regenerate the ID "after any privilege level change", including login and password change. (OWASP Session Management)
- Enforce expiry on the server. Deleting a cookie alone does not invalidate a stolen ID, and "All session IDs belonging to the same logical authenticated session must be revoked together". (OWASP Session Management)
- "When either timeout expires, the session SHALL be terminated." Cookies "SHALL NOT be relied upon to enforce session timeouts" and "SHOULD contain only an opaque string". ([NIST SP 800-63B-4, session management](https://pages.nist.gov/800-63-4/sp800-63b/session/))

### Option A: tower-sessions with tower-sessions-sqlx-store

What it gets right:

- Cookie defaults are `name = "id"`, `http_only = true`, `same_site = Strict`, `secure = true`. (source `src/service.rs` at tag v0.15.0)
- IDs are i128 values from `rand` (ChaCha12). ([docs](https://docs.rs/tower-sessions/0.15.0/tower_sessions/))
- `Session::cycle_id()` prevents fixation. (source `tower-sessions-core/src/session.rs`)

Why it fits poorly here:

- The Postgres store's table is `(id text, data bytea, expiry_date timestamptz)`, and `data` is MessagePack (`rmp-serde`). (source `sqlx-store/src/postgres_store.rs`) There is no `user_id` column. Two things Pranata needs then require decoding every row:
  - the System module listing a staff member's active sessions;
  - revoking all sessions of a user when they are disabled or change their password.
- `Expiry::OnInactivity` "is computed from the last time the session was _modified_. Reading a session is not considered activity." (source `session.rs`) Sliding expiry therefore needs a write per request anyway, and there is no built-in absolute timeout.
- The SQLx store doesn't delete expired rows by itself. You run `continuously_delete_expired` as a background task. ([docs](https://docs.rs/tower-sessions/0.15.0/tower_sessions/))

### Option B (recommended): a hand-rolled sessions table

It is roughly one migration, one extractor and one middleware, using crates the backend already has (`sqlx`, `axum-extra` cookies) plus `getrandom` and `sha2`.

```sql
create table sessions (
  token_hash        bytea primary key,          -- sha256(token); the raw token only lives in the cookie
  user_id           uuid not null references users(id) on delete cascade,
  remember          boolean not null,
  authenticated_at  timestamptz not null,       -- last full login or re-auth, for "sudo mode"
  last_seen_at      timestamptz not null,
  idle_expires_at   timestamptz not null,
  absolute_expires_at timestamptz not null,
  user_agent        text,
  ip                inet,
  created_at        timestamptz not null default now()
);
create index on sessions (user_id);
```

- **Token:** 32 bytes from `getrandom`, base64url-encoded into the cookie. That is 256 bits, well above OWASP's 64-bit minimum. Store only `sha256(token)`, so a leaked database or backup holds no usable session tokens. (This is a project design choice, not a cited requirement.)
- **Login:** always insert a new row. Never reuse a pre-login token, which prevents fixation.
- **Logout:** delete the row and clear the cookie.
- **Revoke all:** run `delete from sessions where user_id = $1` on password change, on disable, or from the active-sessions screen.
- **Each request:** look up the row by hash and reject it if either expiry has passed. Bump `last_seen_at` and `idle_expires_at`, throttled to at most once per minute to avoid a write on every request.
- **Cleanup:** a periodic `delete from sessions where idle_expires_at < now() or absolute_expires_at < now()`.
- **Binding:** record IP and User-Agent for display and audit, but don't reject a session because they changed. OWASP notes these "cannot be used by web applications to trustingly defend against session attacks". (OWASP Session Management)

### Cookie attributes

`Set-Cookie: __Host-session=<token>; Secure; HttpOnly; SameSite=Lax; Path=/` (plus `Max-Age` only for "remember me")

- `__Host-`: "must be set with Secure, must not have a Domain attribute, and must use Path=/". (OWASP Session Management) The CSRF cheat sheet recommends it alongside `SameSite`. ([OWASP CSRF](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html))
- `SameSite`: OWASP prefers `Strict` and accepts `Lax`. ("Session cookies must explicitly set SameSite=Strict (preferred) or SameSite=Lax.") Use `Lax`: with `Strict`, a staff member who follows a link to the admin area from WhatsApp or email arrives logged out on the first page load. CSRF is handled separately (section 5), and OWASP treats SameSite as defense in depth only.
- TanStack Start's own authentication guide uses the same shape: `'__Host-session'`, `httpOnly: true`, `secure: true`, `sameSite: 'lax'`, `path: '/'`. ([TanStack Start: authentication server primitives](https://tanstack.com/start/latest/docs/framework/react/guide/authentication-server-primitives))
- **Development:** `__Host-` requires `Secure`. Run local development through Caddy with its local HTTPS certificate, or use a plain `session` name only in a debug build.

### Who sets the cookie

- Login, logout and passkey ceremonies are `/api/auth/*` calls made by the browser through Caddy straight to axum, so axum's `Set-Cookie` reaches the browser directly.
- During SSR, TanStack Start reads the incoming `Cookie` header with `getRequestHeader('cookie')` and forwards it on its internal `fetch` to axum. ([TanStack Start: server functions](https://tanstack.com/start/latest/docs/framework/react/guide/server-functions))
- With Option B the cookie value never changes after login, and expiry slides only in the database, so SSR never has to relay `Set-Cookie` back to the browser. This is the main simplification over libraries that rewrite the cookie on every touch.
- axum must only be reachable through Caddy or the internal container network, never published directly.

## 3. Password hashing: argon2 (RustCrypto)

- OWASP lists equivalent Argon2id minimums. One of them is "m=19456 (19 MiB), t=2, p=1 (Do not use with Argon2i)". Choosing among them is a CPU-versus-RAM tradeoff. ([OWASP Password Storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html))
- `argon2` 0.6.0 defaults to Argon2id v19 with `DEFAULT_M_COST = 19 * 1024`, `DEFAULT_T_COST = 2`, `DEFAULT_P_COST = 1` and `DEFAULT_OUTPUT_LEN = 32`, matching that OWASP row exactly. (source `src/params.rs` in the 0.6.0 crate; [docs](https://docs.rs/argon2/0.6.0/argon2/))
- The API is `Argon2::hash_password(password)` (salt generated through the default `getrandom` feature) returning a PHC string `$argon2id$v=19$...`, verified with `PasswordHash::new` and `verify_password`. Verification uses the parameters stored in the hash, not the instance's. ([docs](https://docs.rs/argon2/0.6.0/argon2/))

**Recommendations:**

1. Build the hasher with explicit `Params` (19456, 2, 1) instead of `Argon2::default()`, so a future crate default change can't silently alter the work factor.
2. Run hashing and verification inside `tokio::task::spawn_blocking`. Each hash takes about 19 MiB of RAM and tens of milliseconds of CPU, which must not stall the async runtime. On a small VPS, bound concurrent hashes (for example with a `tokio::sync::Semaphore`). Login rate limiting is part of #9.
3. **Rehash on login:** after a successful verify, if the stored PHC parameters differ from the current ones, rehash and update the row. OWASP: "wait until the user next authenticates, then re-hash their password with the new work factor."
4. **No pepper for now.** `Argon2::new_with_secret` exists. But on a single VPS the pepper would sit on the same host as the database, and OWASP notes a pepper "cannot be changed without knowledge of a user's password". The cost isn't worth it at this scale.
5. When the email is unknown, still verify against a fixed dummy hash, so response time doesn't reveal which emails are staff accounts.
6. Argon2 has no bcrypt-style 72-byte limit (OWASP Password Storage), but cap the input length to bound hashing cost. The exact limit is decided in #9.
7. Tune once on the production VPS class. OWASP: aim for a hash calculation under one second. If there is spare RAM, a higher-memory OWASP row is equally valid.

## 4. "Remember me" semantics

The prototype's login has an "Ingat saya" (remember me) checkbox (#9). The sources constrain it as follows:

- "Cookies and similar 'remember my browser' features SHALL NOT be used instead of authentication." The only exception is AAL2 reauthentication after an inactivity timeout, within the overall limit. ([NIST SP 800-63B-4, session management](https://pages.nist.gov/800-63-4/sp800-63b/session/))
- NIST reauthentication limits: AAL1 overall "no more than 30 days", inactivity optional. AAL2 overall "no more than 24 hours", inactivity "no more than 1 hour". ([NIST SP 800-63B-4, AALs](https://pages.nist.gov/800-63-4/sp800-63b/aal/))
- OWASP idle timeouts: "2-5 minutes for high-value applications and 15-30 minutes for low risk applications". Absolute: "between 4 and 8 hours" for an office worker. OWASP says nothing specific about remember-me. (OWASP Session Management)

**Recommended meaning:** "remember me" makes the same server-side session longer-lived on that one browser. It never skips a factor, and it never issues a second long-lived token type.

|                           | Without "remember me"                                   | With "remember me"              |
| ------------------------- | ------------------------------------------------------- | ------------------------------- |
| Cookie                    | Session cookie (no `Max-Age`)                           | `Max-Age` = absolute lifetime   |
| Idle timeout (server)     | 30 min                                                  | 7 days                          |
| Absolute timeout (server) | 12 h (one working day)                                  | 30 days (the NIST AAL1 ceiling) |
| Sensitive actions         | Need re-auth if `authenticated_at` is older than 15 min | Same                            |

- **Sensitive actions ("sudo mode")** are those that touch personal data in bulk or change access: exporting the population register, managing staff accounts and roles, adding or removing passkeys, changing a password. They require `authenticated_at` within the last 15 minutes. Otherwise the UI asks for a passkey or password again, and success updates `authenticated_at` without creating a new session. This keeps a stolen or forgotten 30-day browser from being a full key to resident data. It is one column plus one extractor check.
- The active-sessions list in the System module shows each session's User-Agent, IP, last seen time and a revoke button. Revoking deletes the row.
- The timeout values above are suggested defaults. #9 owns the final numbers. Making them Installation settings is not recommended: fixed constants are simpler.

## 5. CSRF with same-origin SSR and API behind Caddy

Topology: Caddy serves `https://<village-domain>`, routes `/api/*` to axum and everything else to TanStack Start. Browser mutations go to `/api/*` directly through the Orval-generated client.

The sources say:

- "If your software targets only modern browsers, you may rely on Fetch Metadata headers", but "a fallback to standard origin verification headers is a mandatory requirement". Reject non-safe methods when `Sec-Fetch-Site` is `cross-site`, allow `same-origin`, and treat `same-site` conservatively. A missing `Origin` header and `Origin: null` are not proof of same origin. The page puts `Sec-Fetch-*` support in all major browsers at over 98% since March 2023. ([OWASP CSRF](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html))
- "SameSite is useful as a defense-in-depth control but it does not replace a proper CSRF defense." (OWASP CSRF)
- HTML forms can only send "simple" content types (`application/x-www-form-urlencoded`, `multipart/form-data`, `text/plain`). Disallowing them is a mitigation. (OWASP CSRF)
- axum's `Json` extractor rejects requests that lack "a `Content-Type: application/json` (or similar) header". ([axum `Json`](https://docs.rs/axum/0.8.9/axum/struct.Json.html))
- TanStack Start: "Server functions are same-origin RPC endpoints." `createCsrfMiddleware()` "verifies same-origin browser request metadata with `Sec-Fetch-Site`, `Origin`, or `Referer` headers and rejects requests that cannot be proven same-origin". If `src/start.ts` exists, it must be added explicitly with `filter: (ctx) => ctx.handlerType === 'serverFn'`. ([TanStack Start: middleware](https://tanstack.com/start/latest/docs/framework/react/guide/middleware), [server functions](https://tanstack.com/start/latest/docs/framework/react/guide/server-functions))

**Recommendation: no CSRF tokens. Four layers:**

1. **An axum middleware on all `/api` routes**, about 20 lines, unit tested. For any method other than GET, HEAD or OPTIONS:
   - If `Sec-Fetch-Site` is present, allow only `same-origin`.
   - If it is absent, require an `Origin` header whose full origin (scheme, host, port) equals the configured `PUBLIC_ORIGIN` of the Installation.
   - Otherwise, respond 403.
2. **JSON-only request bodies** through `Json<T>`. File uploads (`multipart/form-data`) are a simple content type, so they rely on layer 1, which is enough.
3. **`SameSite=Lax` and the `__Host-` prefix** on the session cookie, as defense in depth.
4. **TanStack Start `createCsrfMiddleware`** registered in `src/start.ts` for server functions. Keep SSR route loaders read-only (GET). If a server function must ever mutate through axum, forward the browser's `Origin` and `Sec-Fetch-Site`, which `createCsrfMiddleware` has already validated, on the internal call.

Never change state in a GET handler. OWASP: "Do not use GET requests for state changing operations."

**Login CSRF:** the login endpoints are covered by the same Origin and Fetch Metadata check, which needs no pre-session token. A successful login always creates a fresh session row. (OWASP CSRF, "login CSRF")

## Open questions handed to #9

- Final idle and absolute timeouts, and the sudo-mode window. The values above are suggestions.
- Whether passkeys can fully replace passwords for a staff member. If they can, a domain change still needs a recovery path.
- The password length cap, rate limiting and lockout.
- The client-side WebAuthn JSON encoding helper (`PublicKeyCredential.parseCreationOptionsFromJSON` or a small library). Browser support was not verified in this research.

## Sources

- webauthn-rs crate docs 0.6.2: https://docs.rs/webauthn-rs/0.6.2/webauthn_rs/ and source of `webauthn-rs-0.6.2.crate` (`src/lib.rs`, `src/interface.rs`)
- webauthn-rs README: https://github.com/kanidm/webauthn-rs
- tower-sessions docs 0.15.0: https://docs.rs/tower-sessions/0.15.0/tower_sessions/ and source at tag `v0.15.0` (`src/service.rs`, `tower-sessions-core/src/session.rs`)
- tower-sessions-stores source (`sqlx-store/src/postgres_store.rs`): https://github.com/maxcountryman/tower-sessions-stores
- argon2 docs 0.6.0: https://docs.rs/argon2/0.6.0/argon2/ and source of `argon2-0.6.0.crate` (`src/params.rs`)
- axum `Json` 0.8.9: https://docs.rs/axum/0.8.9/axum/struct.Json.html
- OWASP Password Storage Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- OWASP Session Management Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
- OWASP CSRF Prevention Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
- NIST SP 800-63B-4: https://pages.nist.gov/800-63-4/sp800-63b/session/ and https://pages.nist.gov/800-63-4/sp800-63b/aal/
- TanStack Start docs: https://tanstack.com/start/latest/docs/framework/react/guide/server-functions, https://tanstack.com/start/latest/docs/framework/react/guide/middleware, https://tanstack.com/start/latest/docs/framework/react/guide/authentication-server-primitives
- Registries: https://crates.io/api/v1/crates/{name} and https://registry.npmjs.org/@tanstack/react-start
