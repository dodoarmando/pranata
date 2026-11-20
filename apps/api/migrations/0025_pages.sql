-- Pages: undated content under one of the public site's fixed parent menus. A Draft may be
-- incomplete while it autosaves; a Published one has every required field.
CREATE TYPE page_menu AS ENUM ('profile', 'government', 'services');

CREATE TABLE pages (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title text NOT NULL,
    -- Unique among Pages; the slug and menu make the URL, so both lock once first published.
    slug text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    menu page_menu NOT NULL,
    -- Smallest first within the menu.
    sort_order integer NOT NULL,
    summary text,
    cover_file_id uuid REFERENCES files,
    cover_caption text,
    cover_alt text,
    -- Tiptap JSON, validated against the allowlist in `rich_text`.
    body jsonb NOT NULL,
    author_id uuid NOT NULL REFERENCES users,
    updated_at timestamptz NOT NULL DEFAULT now(),
    published boolean NOT NULL DEFAULT false,
    first_published_at timestamptz,
    CHECK (NOT published OR (first_published_at IS NOT NULL AND summary IS NOT NULL
        AND (cover_file_id IS NULL OR cover_alt IS NOT NULL)))
);
CREATE INDEX pages_published ON pages (menu, sort_order) WHERE published;
