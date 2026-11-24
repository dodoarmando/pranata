-- The Installation's single Village (D-02). `id = 1` enforces one row.
-- `overview` and `hero_asset_id` are filled by the content tasks that follow;
-- hero_asset_id gets its foreign key when media_assets exists.
CREATE TABLE village (
    id            smallint    PRIMARY KEY CHECK (id = 1),
    code          text        NOT NULL CHECK (code ~ '^[0-9]{2}\.[0-9]{2}\.[0-9]{2}\.[0-9]{4}$'),
    name          text        NOT NULL,
    district      text        NOT NULL,
    regency       text        NOT NULL,
    province      text        NOT NULL,
    time_zone     text        NOT NULL CHECK (time_zone IN ('Asia/Jakarta', 'Asia/Makassar', 'Asia/Jayapura')),
    overview      jsonb,
    hero_asset_id uuid,
    updated_at    timestamptz NOT NULL DEFAULT now()
);

-- The app role edits the profile but never creates or removes the Village;
-- `pranata village init` runs with the owner role.
GRANT SELECT, UPDATE ON village TO pranata_app;
REVOKE INSERT, DELETE, TRUNCATE ON village FROM pranata_app;
