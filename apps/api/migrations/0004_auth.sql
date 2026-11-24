-- Users, server-side sessions and Access Links (D-30, D-32).
-- In R0 every User has full access; roles arrive with plan P2.

-- Users are deactivated, never deleted (D-21), so the app role gets no DELETE.
-- Usernames are stored lower case and are never reused.
CREATE TABLE users (
    id            uuid        PRIMARY KEY DEFAULT uuidv7(),
    username      text        NOT NULL UNIQUE CHECK (username = lower(username) AND username ~ '^[a-z0-9][a-z0-9._-]{2,63}$'),
    display_name  text        NOT NULL CHECK (btrim(display_name) <> ''),
    password_hash text,
    is_active     boolean     NOT NULL DEFAULT true,
    created_at    timestamptz NOT NULL DEFAULT now()
);

GRANT SELECT, INSERT, UPDATE ON users TO pranata_app;
REVOKE DELETE, TRUNCATE ON users FROM pranata_app;

-- The cookie holds the random token; only its SHA-256 is stored, so a leaked
-- table cannot be replayed. Times come from the application clock.
CREATE TABLE sessions (
    token_hash   bytea       PRIMARY KEY,
    user_id      uuid        NOT NULL REFERENCES users (id),
    csrf_token   text        NOT NULL,
    created_at   timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL,
    expires_at   timestamptz NOT NULL,
    remember     boolean     NOT NULL
);

CREATE INDEX sessions_user_idx ON sessions (user_id);
CREATE INDEX sessions_expires_idx ON sessions (expires_at);

GRANT SELECT, INSERT, UPDATE, DELETE ON sessions TO pranata_app;
REVOKE TRUNCATE ON sessions FROM pranata_app;

-- Single-use, short-lived links an Administrator hands over (ADR 0007). Used
-- links stay as a record, so the app role cannot delete them.
CREATE TABLE access_links (
    token_hash bytea       PRIMARY KEY,
    user_id    uuid        NOT NULL REFERENCES users (id),
    created_by uuid        REFERENCES users (id),
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at    timestamptz
);

CREATE INDEX access_links_user_idx ON access_links (user_id);

GRANT SELECT, INSERT, UPDATE ON access_links TO pranata_app;
REVOKE DELETE, TRUNCATE ON access_links FROM pranata_app;

-- Audit entries must never be lost, and users are never deleted (D-21), so a
-- foreign key on the actor cannot orphan or cascade away history. It only
-- guarantees that every recorded actor is a real User. NULL stays valid for
-- system actors such as the install CLI.
ALTER TABLE audit_entries
    ADD CONSTRAINT audit_entries_actor_fkey FOREIGN KEY (actor) REFERENCES users (id);
