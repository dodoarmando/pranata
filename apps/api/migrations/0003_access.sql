-- Users, their Roles, Access Links and the Audit Log.

CREATE FUNCTION reject_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% on % is not allowed', TG_OP, TG_TABLE_NAME;
END
$$;

CREATE TABLE roles (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL UNIQUE,
    -- The built-in, locked Administrator Role, which holds every Permission.
    administrator boolean NOT NULL DEFAULT false
);
CREATE UNIQUE INDEX roles_one_administrator ON roles (administrator) WHERE administrator;
INSERT INTO roles (name, administrator) VALUES ('Administrator', true);

CREATE TABLE users (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    -- Immutable and, since Users are never deleted, never reused.
    username text NOT NULL UNIQUE CHECK (username ~ '^[a-z0-9._-]{3,32}$'),
    role_id uuid NOT NULL REFERENCES roles,
    -- Argon2id PHC string; NULL until the User sets one through an Access Link.
    password_hash text
);
CREATE TRIGGER users_never_deleted BEFORE DELETE ON users
    FOR EACH ROW EXECUTE FUNCTION reject_change();
CREATE TRIGGER users_username_immutable BEFORE UPDATE OF username ON users
    FOR EACH ROW WHEN (OLD.username IS DISTINCT FROM NEW.username) EXECUTE FUNCTION reject_change();

CREATE TABLE access_links (
    -- SHA-256 of the token; the token itself is shown once and never stored.
    token_hash bytea PRIMARY KEY,
    -- One live link per User: creating a new one replaces the old.
    user_id uuid NOT NULL UNIQUE REFERENCES users,
    expires_at timestamptz NOT NULL
);
CREATE INDEX access_links_expires_at ON access_links (expires_at);

CREATE TABLE audit_entries (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    at timestamptz NOT NULL DEFAULT now(),
    actor text NOT NULL CHECK (actor IN ('user', 'system', 'anonymous')),
    actor_id uuid REFERENCES users CHECK ((actor = 'user') = (actor_id IS NOT NULL)),
    action text NOT NULL,
    object_type text NOT NULL,
    object_id text,
    details jsonb NOT NULL DEFAULT '{}'
);
CREATE INDEX audit_entries_at ON audit_entries (at);

-- Append-only: the application's role keeps only INSERT and SELECT. The triggers
-- also hold for superusers and table owners, who bypass or can re-grant privileges.
REVOKE ALL ON audit_entries FROM PUBLIC, CURRENT_USER;
GRANT INSERT, SELECT ON audit_entries TO CURRENT_USER;
CREATE TRIGGER audit_entries_append_only BEFORE UPDATE OR DELETE ON audit_entries
    FOR EACH ROW EXECUTE FUNCTION reject_change();
CREATE TRIGGER audit_entries_no_truncate BEFORE TRUNCATE ON audit_entries
    FOR EACH STATEMENT EXECUTE FUNCTION reject_change();
