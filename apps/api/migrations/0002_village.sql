-- The Installation's Village: one row, written only by `pranata village set-code`.
CREATE TABLE village (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    code text NOT NULL CHECK (code ~ '^[0-9]{6}[23][0-9]{3}$'),
    name text NOT NULL,
    district text NOT NULL,
    regency text NOT NULL,
    province text NOT NULL
);
