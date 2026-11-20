-- A stored PDF's size in bytes, shown beside its download link; null for images.
ALTER TABLE files ADD COLUMN byte_size bigint;

-- The fixed catalog of Transparency Document types.
CREATE TYPE document_type AS ENUM (
    'apbdes', 'apbdes_amendment', 'realisation_report', 'rkpdes', 'rpjmdes',
    'village_regulation', 'other'
);

-- Transparency Documents: official planning, budget and regulation PDFs published for download.
CREATE TABLE transparency_documents (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title text NOT NULL,
    document_type document_type NOT NULL,
    fiscal_year integer NOT NULL,
    file_id uuid NOT NULL REFERENCES files,
    published boolean NOT NULL DEFAULT false,
    -- The last save, which orders the dashboard's Drafts.
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX transparency_documents_published ON transparency_documents (fiscal_year DESC)
    WHERE published;
