-- Important Contacts: phone numbers residents may need, grouped by the Village's ordered contact
-- categories. One a contact is filed under is kept.
CREATE TABLE contact_categories (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL,
    -- Smallest first.
    sort_order integer NOT NULL
);
CREATE UNIQUE INDEX contact_categories_name ON contact_categories (lower(name));

CREATE TABLE important_contacts (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    name text NOT NULL,
    category_id uuid NOT NULL REFERENCES contact_categories,
    phone text NOT NULL,
    note text,
    -- Smallest first within the category.
    sort_order integer NOT NULL,
    published boolean NOT NULL DEFAULT false
);
CREATE INDEX important_contacts_category ON important_contacts (category_id, sort_order);
