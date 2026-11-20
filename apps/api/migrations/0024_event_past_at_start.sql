-- An Event without an end is past once its start has gone by, not at the end of that day: a
-- morning meeting must not stay "upcoming" all evening. Staff set an end for longer listings.
CREATE OR REPLACE FUNCTION event_is_past(starts_at timestamp, ends_at timestamp, now timestamp)
RETURNS boolean LANGUAGE sql IMMUTABLE
RETURN coalesce(ends_at, starts_at) < now;
