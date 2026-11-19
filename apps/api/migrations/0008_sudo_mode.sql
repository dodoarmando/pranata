-- When the session's User last gave their password; sudo mode lasts 15 minutes from it.
ALTER TABLE sessions ADD COLUMN authenticated_at timestamptz NOT NULL DEFAULT now();
