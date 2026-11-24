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

-- Uploaded files (D-15, D-16). Content-addressed: the SHA-256 of the upload is
-- unique, so the same file is stored once. An image starts `pending` and turns
-- `ready` (or `failed`) when its job has run; a document is `ready` at once.
-- Dimensions are those of the stored original after the EXIF orientation is
-- applied, and are unknown until the job has run.
CREATE TABLE media_assets (
    id            uuid        PRIMARY KEY DEFAULT uuidv7(),
    sha256        bytea       NOT NULL UNIQUE CHECK (octet_length(sha256) = 32),
    kind          text        NOT NULL CHECK (kind IN ('image', 'document')),
    mime          text        NOT NULL,
    original_name text        NOT NULL,
    bytes         bigint      NOT NULL CHECK (bytes > 0),
    width         integer     CHECK (width > 0),
    height        integer     CHECK (height > 0),
    status        text        NOT NULL CHECK (status IN ('pending', 'ready', 'failed')),
    created_by    uuid        NOT NULL REFERENCES users (id),
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX media_assets_created_idx ON media_assets (created_at DESC);

-- Public WebP renditions, one per width. `path` is relative to the media
-- directory and always starts with `public/`.
CREATE TABLE media_variants (
    asset_id uuid    NOT NULL REFERENCES media_assets (id),
    width    integer NOT NULL CHECK (width > 0),
    path     text    NOT NULL,
    bytes    bigint  NOT NULL CHECK (bytes > 0),
    PRIMARY KEY (asset_id, width)
);
