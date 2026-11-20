-- Events: a Draft may be incomplete while it autosaves; a Published one has every required
-- field. A Cancelled one stays listed. Upcoming or past is computed from the time, never stored.
CREATE TABLE events (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    title text NOT NULL,
    category_id uuid REFERENCES categories,
    -- Wall-clock times in the Village's time zone, like Announcement dates.
    starts_at timestamp,
    ends_at timestamp CHECK (ends_at >= starts_at),
    location text,
    description text,
    cancelled boolean NOT NULL DEFAULT false,
    author_id uuid NOT NULL REFERENCES users,
    updated_at timestamptz NOT NULL DEFAULT now(),
    published boolean NOT NULL DEFAULT false,
    first_published_at timestamptz,
    CHECK (NOT published OR (first_published_at IS NOT NULL AND category_id IS NOT NULL
        AND starts_at IS NOT NULL AND location IS NOT NULL))
);
CREATE INDEX events_published ON events (starts_at) WHERE published;

-- Past once its end has gone by or, without an end, once the day it starts on is over; `now`
-- is the Village's wall clock.
CREATE FUNCTION event_is_past(starts_at timestamp, ends_at timestamp, now timestamp)
RETURNS boolean LANGUAGE sql IMMUTABLE
RETURN CASE WHEN ends_at IS NULL THEN starts_at::date < now::date ELSE ends_at < now END;
