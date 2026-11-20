-- Bodies outside the Village government, such as the BPD, LPM, PKK or BUMDes.
CREATE TABLE institutions (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL,
    abbreviation text,
    description text NOT NULL,
    logo_file_id uuid REFERENCES files
);

-- An Institution's members in order; no tenure history, so a change replaces them all.
CREATE TABLE institution_members (
    institution_id uuid NOT NULL REFERENCES institutions ON DELETE CASCADE,
    sort_order integer NOT NULL,
    name text NOT NULL,
    role text NOT NULL,
    PRIMARY KEY (institution_id, sort_order)
);
