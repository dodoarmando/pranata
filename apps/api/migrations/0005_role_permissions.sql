-- The Permissions a Role holds, from the catalog fixed in code; the Administrator holds every one regardless.
ALTER TABLE roles ADD COLUMN permissions text[] NOT NULL DEFAULT '{}';
