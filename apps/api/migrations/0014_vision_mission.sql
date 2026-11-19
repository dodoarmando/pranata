-- The Village's one current Vision and Mission; earlier versions live only in the Audit Log.
ALTER TABLE village
    ADD COLUMN vision text,
    ADD COLUMN missions text[],
    ADD COLUMN rpjmdes_start_year integer,
    ADD COLUMN rpjmdes_end_year integer,
    ADD CHECK (num_nulls(vision, missions, rpjmdes_start_year, rpjmdes_end_year) IN (0, 4)),
    ADD CHECK (rpjmdes_start_year < rpjmdes_end_year);
