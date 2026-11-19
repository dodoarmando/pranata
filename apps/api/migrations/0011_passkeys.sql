-- Users' passkeys, each the webauthn-rs `Passkey` serialised as the crate recommends.
CREATE TABLE passkeys (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id uuid NOT NULL REFERENCES users,
    -- Unique across Users, as WebAuthn requires.
    credential_id bytea NOT NULL UNIQUE,
    passkey jsonb NOT NULL,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz
);
CREATE INDEX passkeys_user_id ON passkeys (user_id);
