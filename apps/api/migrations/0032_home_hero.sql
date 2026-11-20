-- The home page hero and its Quick Links, saved live by publishers: one row, once first saved.
CREATE TABLE home_hero (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    photo_file_id uuid NOT NULL REFERENCES files,
    photo_alt text NOT NULL,
    title text NOT NULL,
    subtitle text,
    -- The button shows only with both.
    button_label text,
    button_path text,
    -- In order, at most four: [{ "label": "...", "path": "/..." }].
    quick_links jsonb NOT NULL,
    CHECK ((button_label IS NULL) = (button_path IS NULL))
);
