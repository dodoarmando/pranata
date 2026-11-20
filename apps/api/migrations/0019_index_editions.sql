-- The ministry's score for one data-collection year: Indeks Desa since 2024, IDM before it.
CREATE TYPE index_kind AS ENUM ('indeks_desa', 'idm');
-- Mandiri, Maju, Berkembang, Tertinggal or Sangat Tertinggal; stored as entered, never computed.
CREATE TYPE development_status AS ENUM
    ('independent', 'advanced', 'developing', 'underdeveloped', 'very_underdeveloped');

CREATE TABLE index_editions (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    year smallint NOT NULL UNIQUE,
    kind index_kind NOT NULL,
    -- 0–100 with 2 decimals for Indeks Desa, 0–1 with 4 decimals for IDM.
    score numeric(7, 4) NOT NULL CHECK (score BETWEEN 0 AND CASE kind WHEN 'idm' THEN 1 ELSE 100 END),
    status development_status NOT NULL,
    -- Indeks Desa's six dimensions, each optional; IDM has none.
    basic_services numeric(5, 2),
    social numeric(5, 2),
    economic numeric(5, 2),
    environmental numeric(5, 2),
    accessibility numeric(5, 2),
    governance numeric(5, 2),
    CHECK (kind = 'idm' OR score = round(score, 2)),
    CHECK (0 <= least(basic_services, social, economic, environmental, accessibility, governance)),
    CHECK (greatest(basic_services, social, economic, environmental, accessibility, governance) <= 100),
    CHECK (kind = 'indeks_desa'
        OR num_nonnulls(basic_services, social, economic, environmental, accessibility, governance) = 0)
);
