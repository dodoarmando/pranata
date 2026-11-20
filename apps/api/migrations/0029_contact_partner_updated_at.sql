-- The last save of an Important Contact or Partner, which orders the dashboard's Drafts.
ALTER TABLE important_contacts ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE partners ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();
