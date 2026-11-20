-- Albums: an ordered set of photos from one village occasion. A Draft may be incomplete while
-- it autosaves; publishing also needs a photo and alt text on every photo, checked on save.
CREATE TABLE albums (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title text NOT NULL,
    -- Unique among Albums; locked once first published, so shared links never break.
    slug text NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    description text,
    -- Inclusive, in the Village's time zone, like Announcement dates.
    occasion_start date,
    occasion_end date CHECK (occasion_end >= occasion_start),
    -- One of the Album's photos; null means the first.
    cover_file_id uuid REFERENCES files,
    author_id uuid NOT NULL REFERENCES users,
    updated_at timestamptz NOT NULL DEFAULT now(),
    published boolean NOT NULL DEFAULT false,
    first_published_at timestamptz,
    CHECK (NOT published OR (first_published_at IS NOT NULL AND occasion_start IS NOT NULL))
);
CREATE INDEX albums_published ON albums (occasion_start DESC) WHERE published;

CREATE TABLE album_photos (
    album_id uuid NOT NULL REFERENCES albums ON DELETE CASCADE,
    -- From 0, in the order the public sees.
    position integer NOT NULL,
    file_id uuid NOT NULL REFERENCES files,
    caption text,
    -- Required to publish.
    alt text,
    PRIMARY KEY (album_id, position),
    UNIQUE (album_id, file_id)
);
