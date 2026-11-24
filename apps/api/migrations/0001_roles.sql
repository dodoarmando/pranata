-- The roles themselves are created at cluster level (infra/dev/postgres-init.sql
-- in development and CI, the provisioning playbook in production). This
-- migration only wires up privileges: the owner role owns the schema and the
-- app role gets DML on whatever the owner creates later.
GRANT USAGE ON SCHEMA public TO pranata_app;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO pranata_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO pranata_app;
