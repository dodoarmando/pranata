-- Complaints: what residents send through the public form, read only by staff, and followed by
-- the sender with a Complaint Code.
CREATE TABLE complaint_topics (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL,
    -- Smallest first.
    sort_order integer NOT NULL,
    -- An inactive Topic keeps its Complaints but takes no new ones.
    active boolean NOT NULL DEFAULT true
);
CREATE UNIQUE INDEX complaint_topics_name ON complaint_topics (lower(name));
INSERT INTO complaint_topics (name, sort_order) VALUES
    ('Infrastruktur', 1), ('Pelayanan', 2), ('Bantuan Sosial', 3), ('Keamanan', 4), ('Lainnya', 5);

CREATE TYPE complaint_state AS ENUM ('new', 'in_progress', 'closed', 'rejected');

CREATE TABLE complaints (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    code text NOT NULL UNIQUE,
    topic_id uuid NOT NULL REFERENCES complaint_topics,
    text text NOT NULL CHECK (char_length(text) BETWEEN 1 AND 2000),
    -- The sender's optional Hamlet, name and phone, never shown on the status page.
    hamlet_id uuid REFERENCES region_units,
    name text,
    phone text,
    state complaint_state NOT NULL DEFAULT 'new',
    rejection_reason text CHECK ((state = 'rejected') = (rejection_reason IS NOT NULL)),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX complaints_state ON complaints (state);

CREATE TABLE complaint_responses (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    complaint_id uuid NOT NULL REFERENCES complaints,
    user_id uuid NOT NULL REFERENCES users,
    text text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX complaint_responses_complaint ON complaint_responses (complaint_id, created_at);
CREATE TRIGGER complaint_responses_never_edited BEFORE UPDATE ON complaint_responses
    FOR EACH ROW EXECUTE FUNCTION reject_change();
