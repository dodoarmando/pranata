-- An Audit Entry's per-field diff, `[{ "field", "from", "to", "sensitive"? }]`; sensitive
-- values are masked for readers without `resident.sensitive`.
ALTER TABLE audit_entries ADD COLUMN changes jsonb NOT NULL DEFAULT '[]';
