-- News Posts: a Draft may be incomplete while it autosaves; a Published one has every
-- required field. The public date is the first publication, kept through an unpublish.
CREATE TABLE news_posts (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title text NOT NULL,
    -- Unique among News Posts; locked once first published, so shared links never break.
    slug text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    category_id uuid REFERENCES categories,
    summary text,
    cover_file_id uuid REFERENCES files,
    cover_caption text,
    cover_alt text,
    -- Tiptap JSON, validated against the allowlist in `rich_text`.
    body jsonb NOT NULL,
    reading_minutes integer NOT NULL,
    author_id uuid NOT NULL REFERENCES users,
    updated_at timestamptz NOT NULL DEFAULT now(),
    published boolean NOT NULL DEFAULT false,
    first_published_at timestamptz,
    CHECK (NOT published OR (first_published_at IS NOT NULL AND category_id IS NOT NULL
        AND summary IS NOT NULL AND cover_file_id IS NOT NULL AND cover_alt IS NOT NULL))
);
CREATE INDEX news_posts_published ON news_posts (first_published_at DESC) WHERE published;
