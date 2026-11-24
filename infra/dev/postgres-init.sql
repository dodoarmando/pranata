-- Dev and CI bootstrap, run once by the postgres image on first start.
-- Role creation is cluster-level, so it lives here and not in migrations.
-- Passwords are for local development only; real installations provision
-- their own secrets.

CREATE ROLE pranata_owner LOGIN PASSWORD 'pranata_owner_dev';
CREATE ROLE pranata_app LOGIN PASSWORD 'pranata_app_dev';

-- POSTGRES_DB=pranata creates the database; hand it to the owner role so the
-- owner may create objects in the public schema.
ALTER DATABASE pranata OWNER TO pranata_owner;
