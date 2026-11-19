-- When the session's User last gave their password; sudo mode lasts 15 minutes from it.
-- Sessions from before this column never gave it, so they start outside sudo mode.
ALTER TABLE sessions ADD COLUMN authenticated_at timestamptz NOT NULL DEFAULT '-infinity';
ALTER TABLE sessions ALTER COLUMN authenticated_at SET DEFAULT now();
