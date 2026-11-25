-- Failed sign-ins per account (D-34), counted in a sliding window by the
-- application clock. Rows are append-only: they are inserted, counted and
-- deleted (when the User signs in, redeems an Access Link, or after one day),
-- never updated. Usernames that do not exist are recorded too, so the lock
-- does not reveal whether a User exists; only well-formed usernames are
-- stored, which the length check backs up.
CREATE TABLE login_failures (
    id       bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username text        NOT NULL CHECK (char_length(username) <= 64),
    at       timestamptz NOT NULL
);

CREATE INDEX login_failures_username_at_idx ON login_failures (username, at);

-- Migration 0001's default privileges already grant UPDATE on every new table.
GRANT SELECT, INSERT, DELETE ON login_failures TO pranata_app;
REVOKE UPDATE, TRUNCATE ON login_failures FROM pranata_app;
