-- Which APBDes a summary follows: the original (Murni) or its amendment (Perubahan).
CREATE TYPE budget_version AS ENUM ('original', 'amendment');

-- APBDes Summaries: one fiscal year's budget, and optionally its realisation, in whole rupiah.
CREATE TABLE apbdes_summaries (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    fiscal_year integer NOT NULL UNIQUE,
    version budget_version NOT NULL,
    version_date date NOT NULL,
    -- The budget lines; their shape is checked by the API.
    budget jsonb NOT NULL,
    -- The realised lines with their as-of date, or null before any realisation.
    realisation jsonb,
    published boolean NOT NULL DEFAULT false,
    -- The last save, which orders the dashboard's Drafts.
    updated_at timestamptz NOT NULL DEFAULT now()
);
