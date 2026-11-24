-- Append-only audit log (D-35). The application role may only INSERT and
-- SELECT; the privileges are stated explicitly so the result does not depend
-- on which role's default privileges were in force when the table was created.
CREATE TABLE audit_entries (
    id          uuid        PRIMARY KEY DEFAULT uuidv7(),
    at          timestamptz NOT NULL DEFAULT now(),
    actor       uuid,
    action      text        NOT NULL,
    object_type text        NOT NULL,
    object_id   text        NOT NULL,
    summary     text        NOT NULL,
    changes     jsonb       NOT NULL DEFAULT '[]'::jsonb
);

CREATE INDEX audit_entries_object_idx ON audit_entries (object_type, object_id, at DESC);
CREATE INDEX audit_entries_at_idx ON audit_entries (at DESC);

GRANT SELECT, INSERT ON audit_entries TO pranata_app;
REVOKE UPDATE, DELETE, TRUNCATE ON audit_entries FROM pranata_app;
