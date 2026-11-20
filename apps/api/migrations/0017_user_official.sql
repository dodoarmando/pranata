-- A User may stand for one Official.
ALTER TABLE users ADD COLUMN official_id uuid UNIQUE REFERENCES officials;
