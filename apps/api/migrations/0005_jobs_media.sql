-- Background jobs (D-14). Workers claim one due job with FOR UPDATE SKIP LOCKED,
-- so no job runs on two workers at once. A job that keeps failing ends with
-- `failed_at` set and is never claimed again.
CREATE TABLE jobs (
    id         bigserial   PRIMARY KEY,
    kind       text        NOT NULL,
    payload    jsonb       NOT NULL DEFAULT '{}'::jsonb,
    run_at     timestamptz NOT NULL DEFAULT now(),
    attempts   integer     NOT NULL DEFAULT 0,
    last_error text,
    done_at    timestamptz,
    failed_at  timestamptz
);

-- The claim query only looks at jobs that are still open.
CREATE INDEX jobs_due_idx ON jobs (run_at, id) WHERE done_at IS NULL AND failed_at IS NULL;
