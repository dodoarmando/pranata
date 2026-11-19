-- Signed-in sessions; see `auth::sign_in` for the timeouts.
CREATE TABLE sessions (
    -- SHA-256 of the cookie's token; the token itself is never stored.
    token_hash bytea PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users,
    idle_timeout interval NOT NULL,
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    -- The absolute limit, however active the session is.
    expires_at timestamptz NOT NULL
);
CREATE INDEX sessions_user_id ON sessions (user_id);
CREATE INDEX sessions_expires_at ON sessions (expires_at);
