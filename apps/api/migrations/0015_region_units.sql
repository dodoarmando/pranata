-- The Village's region tree: Hamlets, an optional RW level, and RTs. Region Units are renamed
-- or renumbered in place, deactivated once empty, and never deleted or moved.
ALTER TABLE village
    ADD COLUMN hamlet_label text NOT NULL DEFAULT 'dusun',
    ADD COLUMN has_rw boolean NOT NULL DEFAULT true;

CREATE TYPE region_kind AS ENUM ('hamlet', 'rw', 'rt');

CREATE TABLE region_units (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    kind region_kind NOT NULL,
    -- An RW's Hamlet, or an RT's RW (its Hamlet when the Village has no RW level).
    parent_id uuid REFERENCES region_units,
    -- A Hamlet's free-text name and manual order.
    name text,
    sort_order integer,
    -- An RW's or RT's number and its optional head, never shown publicly.
    number integer CHECK (number BETWEEN 1 AND 999),
    head_name text,
    head_phone text,
    active boolean NOT NULL DEFAULT true,
    CHECK (CASE kind
        WHEN 'hamlet' THEN parent_id IS NULL AND name IS NOT NULL AND sort_order IS NOT NULL
            AND num_nulls(number, head_name, head_phone) = 3
        ELSE parent_id IS NOT NULL AND number IS NOT NULL AND num_nulls(name, sort_order) = 2
    END)
);
CREATE UNIQUE INDEX region_units_rw_number ON region_units (number) WHERE kind = 'rw';
CREATE UNIQUE INDEX region_units_rt_number ON region_units (parent_id, number) WHERE kind = 'rt';
CREATE INDEX region_units_parent ON region_units (parent_id);
CREATE TRIGGER region_units_never_deleted BEFORE DELETE ON region_units
    FOR EACH ROW EXECUTE FUNCTION reject_change();
CREATE TRIGGER region_units_never_moved BEFORE UPDATE OF kind, parent_id ON region_units
    FOR EACH ROW WHEN (OLD.kind IS DISTINCT FROM NEW.kind OR OLD.parent_id IS DISTINCT FROM NEW.parent_id)
    EXECUTE FUNCTION reject_change();
