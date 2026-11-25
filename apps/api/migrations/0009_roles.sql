-- Roles and their Permissions (D-33). A Role is a named set of Permissions from
-- a fixed catalogue; every User holds exactly one Role.

CREATE TABLE roles (
    id               uuid        PRIMARY KEY DEFAULT uuidv7(),
    name             text        NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 50),
    is_administrator boolean     NOT NULL DEFAULT false,
    created_at       timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX roles_name_key ON roles (lower(name));
-- Exactly one built-in Role may be the Administrator Role.
CREATE UNIQUE INDEX roles_one_administrator_key ON roles (is_administrator) WHERE is_administrator;

-- The Administrator Role holds every Permission by definition, including ones
-- added later, so it has no rows in `role_permissions`.
INSERT INTO roles (name, is_administrator) VALUES ('Administrator', true);

-- The check lists the catalogue of the application (`access::Permission`).
CREATE TABLE role_permissions (
    role_id    uuid NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    permission text NOT NULL CHECK (permission IN (
        'village.manage', 'content.write', 'content.publish', 'budget.manage',
        'audit.read', 'user.manage', 'role.manage'
    )),
    PRIMARY KEY (role_id, permission)
);

GRANT SELECT, INSERT, UPDATE, DELETE ON roles TO pranata_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON role_permissions TO pranata_app;
REVOKE TRUNCATE ON roles, role_permissions FROM pranata_app;

-- Every existing User becomes an Administrator, as in R0 where everyone had full
-- access. The column ends up NOT NULL: this breaks `pranata user create-admin` of
-- the previous release after a rollback, and nothing else of it inserts Users.
ALTER TABLE users ADD COLUMN role_id uuid REFERENCES roles (id);
UPDATE users SET role_id = (SELECT id FROM roles WHERE is_administrator);
ALTER TABLE users ALTER COLUMN role_id SET NOT NULL;
CREATE INDEX users_role_idx ON users (role_id);
