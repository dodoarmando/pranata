-- The Village government: Positions, the Officials who hold them, and their Tenures.
CREATE TABLE positions (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    -- Null for a Head-of-Hamlet Position, which is named after its Hamlet.
    name text,
    head_of_village boolean NOT NULL DEFAULT false,
    -- A Head-of-Hamlet Position's Hamlet; created, deactivated and reactivated with it.
    hamlet_id uuid UNIQUE REFERENCES region_units,
    -- The order in the government structure; Head-of-Hamlet Positions follow their Hamlets'.
    sort_order integer,
    active boolean NOT NULL DEFAULT true,
    CHECK ((hamlet_id IS NULL) = (name IS NOT NULL AND sort_order IS NOT NULL)),
    CHECK (NOT (head_of_village AND hamlet_id IS NOT NULL))
);
CREATE UNIQUE INDEX positions_one_head_of_village ON positions (head_of_village) WHERE head_of_village;
CREATE TRIGGER positions_head_and_hamlet_kept BEFORE DELETE ON positions
    FOR EACH ROW WHEN (OLD.head_of_village OR OLD.hamlet_id IS NOT NULL)
    EXECUTE FUNCTION reject_change();
CREATE TRIGGER positions_never_rebound BEFORE UPDATE OF head_of_village, hamlet_id ON positions
    FOR EACH ROW WHEN (OLD.head_of_village IS DISTINCT FROM NEW.head_of_village
        OR OLD.hamlet_id IS DISTINCT FROM NEW.hamlet_id)
    EXECUTE FUNCTION reject_change();

-- Permendagri 84/2015: the head of village, the secretary, three Kaur and three Kasi.
INSERT INTO positions (name, head_of_village, sort_order) VALUES
    ('Kepala Desa', true, 1),
    ('Sekretaris Desa', false, 2),
    ('Kaur Tata Usaha dan Umum', false, 3),
    ('Kaur Keuangan', false, 4),
    ('Kaur Perencanaan', false, 5),
    ('Kasi Pemerintahan', false, 6),
    ('Kasi Kesejahteraan', false, 7),
    ('Kasi Pelayanan', false, 8);
INSERT INTO positions (hamlet_id, active)
    SELECT id, active FROM region_units WHERE kind = 'hamlet';

-- A person in the Village government, not linked to any Resident record.
CREATE TABLE officials (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL,
    -- Nomor Induk Perangkat Desa, never shown publicly.
    nipd text
);
CREATE TRIGGER officials_never_deleted BEFORE DELETE ON officials
    FOR EACH ROW EXECUTE FUNCTION reject_change();

CREATE TYPE tenure_kind AS ENUM ('definitive', 'appointee', 'acting');

CREATE TABLE tenures (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    position_id uuid NOT NULL REFERENCES positions,
    official_id uuid NOT NULL REFERENCES officials,
    -- Definitif, Pj. (appointee) or Plt. (acting); at most one Definitif or Pj. at a time.
    kind tenure_kind NOT NULL,
    start_date date NOT NULL,
    end_date date CHECK (end_date >= start_date),
    -- The appointing decree, never shown publicly.
    decree_number text NOT NULL,
    decree_date date NOT NULL,
    photo_file_id uuid REFERENCES files,
    photo_alt text,
    CHECK ((photo_file_id IS NULL) = (photo_alt IS NULL))
);
CREATE INDEX tenures_position ON tenures (position_id);
CREATE INDEX tenures_official ON tenures (official_id);
CREATE TRIGGER tenures_never_deleted BEFORE DELETE ON tenures
    FOR EACH ROW EXECUTE FUNCTION reject_change();
CREATE TRIGGER tenures_never_moved BEFORE UPDATE OF position_id, official_id ON tenures
    FOR EACH ROW WHEN (OLD.position_id IS DISTINCT FROM NEW.position_id
        OR OLD.official_id IS DISTINCT FROM NEW.official_id)
    EXECUTE FUNCTION reject_change();
