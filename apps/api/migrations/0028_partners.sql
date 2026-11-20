-- Partners: organisations whose logos appear in the home page partner strip, in a manual order.
CREATE TABLE partners (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL,
    logo_file_id uuid NOT NULL REFERENCES files,
    url text,
    -- Smallest first.
    sort_order integer NOT NULL,
    published boolean NOT NULL DEFAULT false
);
CREATE INDEX partners_published ON partners (sort_order) WHERE published;
