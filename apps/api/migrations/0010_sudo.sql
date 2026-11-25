-- Sudo Mode (D-31): when the session last proved the password. Nullable with
-- no default, so release 0.1.0 can still insert sessions after a rollback;
-- null means the Sudo window is closed. Existing sessions are not backfilled.
ALTER TABLE sessions ADD COLUMN reauthenticated_at timestamptz;
