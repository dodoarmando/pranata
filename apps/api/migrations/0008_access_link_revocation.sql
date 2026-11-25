-- An Access Link can be revoked: by a newer link for the same User, or when the
-- User is deactivated. The column is nullable, so the previous release still works.
ALTER TABLE access_links ADD COLUMN revoked_at timestamptz;
