-- The Village's editable profile beside its identity from the Village Code, and the System
-- setting "Nomor Tanda Daftar PSE".
ALTER TABLE village
    ADD COLUMN address text,
    ADD COLUMN postal_code text CHECK (postal_code ~ '^[0-9]{5}$'),
    ADD COLUMN phone text,
    ADD COLUMN email text,
    ADD COLUMN latitude double precision CHECK (latitude BETWEEN -90 AND 90),
    ADD COLUMN longitude double precision CHECK (longitude BETWEEN -180 AND 180),
    ADD COLUMN logo_file_id uuid REFERENCES files,
    ADD COLUMN pse_number text,
    ADD CHECK ((latitude IS NULL) = (longitude IS NULL));
