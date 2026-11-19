-- Users are deactivated, never deleted; an inactive User cannot sign in or keep a session.
ALTER TABLE users ADD COLUMN active boolean NOT NULL DEFAULT true;
