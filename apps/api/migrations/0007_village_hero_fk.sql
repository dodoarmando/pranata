-- The hero photo of the Village profile must be an uploaded asset. The
-- application further requires it to be a ready image before it sets it.
-- pranata_app already holds UPDATE on village (0003), so no grant changes.
ALTER TABLE village
    ADD CONSTRAINT village_hero_asset_id_fkey
    FOREIGN KEY (hero_asset_id) REFERENCES media_assets (id);
