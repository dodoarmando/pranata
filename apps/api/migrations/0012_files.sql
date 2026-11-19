-- Files on the local volume, one directory each, written once. Owners reference a row; one
-- no owner references is swept once `sweep_after` passes.
CREATE TABLE files (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    created_at timestamptz NOT NULL DEFAULT now(),
    sweep_after timestamptz NOT NULL DEFAULT now() + interval '24 hours'
);
