-- The head-of-village welcome on the home page, saved live by publishers: one row, once first
-- saved. It names the signer's Tenure, which may end after it is chosen.
CREATE TABLE home_welcome (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    photo_file_id uuid NOT NULL REFERENCES files,
    photo_alt text NOT NULL,
    message text NOT NULL,
    signer_tenure_id uuid NOT NULL REFERENCES tenures
);
