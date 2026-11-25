-- Passkeys (D-30, D-32): WebAuthn credentials a User signs in with instead of
-- a password. A Passkey may be deleted (D-21 does not cover it); the Audit
-- Entry keeps the record. `passkey` is webauthn-rs's serialised credential:
-- its public key and signature counter. Times come from the application clock.
CREATE TABLE passkeys (
    id            uuid        PRIMARY KEY DEFAULT uuidv7(),
    user_id       uuid        NOT NULL REFERENCES users (id),
    credential_id bytea       NOT NULL UNIQUE,
    name          text        NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 50),
    passkey       jsonb       NOT NULL,
    created_at    timestamptz NOT NULL,
    last_used_at  timestamptz
);

CREATE INDEX passkeys_user_idx ON passkeys (user_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON passkeys TO pranata_app;
REVOKE TRUNCATE ON passkeys FROM pranata_app;

-- The server half of a WebAuthn ceremony in progress. The state never goes to
-- the client; a row lives five minutes and is deleted when it is used. A
-- sign-in challenge belongs to nobody yet, so `user_id` is null for it.
CREATE TABLE webauthn_challenges (
    id         uuid        PRIMARY KEY DEFAULT uuidv7(),
    kind       text        NOT NULL CHECK (kind IN ('register', 'sign_in', 'sudo')),
    user_id    uuid        REFERENCES users (id),
    state      jsonb       NOT NULL,
    expires_at timestamptz NOT NULL
);

CREATE INDEX webauthn_challenges_expires_idx ON webauthn_challenges (expires_at);

GRANT SELECT, INSERT, UPDATE, DELETE ON webauthn_challenges TO pranata_app;
REVOKE TRUNCATE ON webauthn_challenges FROM pranata_app;
