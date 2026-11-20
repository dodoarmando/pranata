-- Public search over News Posts, Announcements, Pages and Transparency Documents, stemmed with
-- the `indonesian` config so "membangun" finds "pembangunan". Weights: title A, summary B,
-- body C; a Rich Text body counts only its text nodes, never its marks, links or embeds.
CREATE FUNCTION content_search(title text, summary text, body jsonb) RETURNS tsvector
LANGUAGE sql IMMUTABLE PARALLEL SAFE
RETURN setweight(to_tsvector('indonesian', title), 'A')
    || setweight(to_tsvector('indonesian', coalesce(summary, '')), 'B')
    || setweight(to_tsvector('indonesian',
        jsonb_path_query_array(body, 'strict $.** ? (@.type == "text").text')), 'C');

ALTER TABLE news_posts ADD COLUMN search tsvector NOT NULL
    GENERATED ALWAYS AS (content_search(title, summary, body)) STORED;
CREATE INDEX news_posts_search ON news_posts USING gin (search);

ALTER TABLE announcements ADD COLUMN search tsvector NOT NULL
    GENERATED ALWAYS AS (content_search(title, summary, body)) STORED;
CREATE INDEX announcements_search ON announcements USING gin (search);

ALTER TABLE pages ADD COLUMN search tsvector NOT NULL
    GENERATED ALWAYS AS (content_search(title, summary, body)) STORED;
CREATE INDEX pages_search ON pages USING gin (search);

-- The type labels as the public site writes them (`documentTypeLabels` in the web app).
ALTER TABLE transparency_documents ADD COLUMN search tsvector NOT NULL GENERATED ALWAYS AS (
    setweight(to_tsvector('indonesian', title), 'A')
    || setweight(to_tsvector('indonesian', CASE document_type
        WHEN 'apbdes' THEN 'APBDes'
        WHEN 'apbdes_amendment' THEN 'APBDes Perubahan'
        WHEN 'realisation_report' THEN 'Laporan realisasi'
        WHEN 'rkpdes' THEN 'RKPDes'
        WHEN 'rpjmdes' THEN 'RPJMDes'
        WHEN 'village_regulation' THEN 'Perdes'
        ELSE 'Lainnya' END || ' ' || fiscal_year), 'B')
) STORED;
CREATE INDEX transparency_documents_search ON transparency_documents USING gin (search);

-- What a resident typed, as `websearch_to_tsquery` reads it, with its last word a prefix so a
-- half-typed word still matches. The lexemes are already stemmed, so `simple` keeps them as is.
CREATE FUNCTION search_query(typed text) RETURNS tsquery LANGUAGE sql IMMUTABLE PARALLEL SAFE
RETURN to_tsquery('simple',
    regexp_replace(websearch_to_tsquery('indonesian', typed)::text, '''$', ''':*'));

CREATE TYPE search_kind AS ENUM ('news_post', 'announcement', 'page', 'transparency_document');

-- Every Published item search can find (ended Announcements included), with the date it is
-- ordered and shown by. A Transparency Document has no publication date, so its creation.
-- `id` only breaks ties, so a page never repeats or skips an item.
CREATE VIEW search_items AS
    SELECT id, 'news_post'::search_kind AS kind, title, summary, slug, NULL::page_menu AS menu,
        NULL::uuid AS file_id, NULL::document_type AS document_type, NULL::integer AS fiscal_year,
        first_published_at AS dated_at, search
    FROM news_posts WHERE published
    UNION ALL
    -- Only an Announcement with a body has a page of its own.
    SELECT id, 'announcement', title, summary, CASE WHEN has_body THEN slug END, NULL, NULL, NULL,
        NULL, first_published_at, search
    FROM announcements WHERE published
    UNION ALL
    SELECT id, 'page', title, summary, slug, menu, NULL, NULL, NULL, first_published_at, search
    FROM pages WHERE published
    UNION ALL
    SELECT id, 'transparency_document', title, NULL, NULL, NULL, file_id, document_type,
        fiscal_year, uuid_extract_timestamp(id), search
    FROM transparency_documents WHERE published;
