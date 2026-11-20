-- Announcements: a Draft may be incomplete while it autosaves; a Published one has every
-- required field. Whether one is in effect is computed from its dates, never stored.
CREATE TABLE announcements (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title text NOT NULL,
    -- Unique among Announcements; locked once first published, so shared links never break.
    slug text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    summary text,
    -- Tiptap JSON, validated against the allowlist in `rich_text`; a blank one is stored empty.
    body jsonb NOT NULL,
    -- Only an Announcement with a body has a detail page.
    has_body boolean NOT NULL GENERATED ALWAYS AS (jsonb_array_length(body -> 'content') > 0) STORED,
    -- Inclusive, in the Village's time zone.
    start_date date,
    end_date date CHECK (end_date >= start_date),
    author_id uuid NOT NULL REFERENCES users,
    updated_at timestamptz NOT NULL DEFAULT now(),
    published boolean NOT NULL DEFAULT false,
    first_published_at timestamptz,
    CHECK (NOT published OR (first_published_at IS NOT NULL AND summary IS NOT NULL
        AND start_date IS NOT NULL AND end_date IS NOT NULL))
);
CREATE INDEX announcements_published ON announcements (start_date DESC) WHERE published;
