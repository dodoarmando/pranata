-- The Village's one list of labels for News Posts and, later, Events; one in use is kept.
CREATE TABLE categories (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL
);
CREATE UNIQUE INDEX categories_name ON categories (lower(name));
