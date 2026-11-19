-- The browser and address a session was started from, shown to its User on "Akun saya".
ALTER TABLE sessions ADD COLUMN user_agent text, ADD COLUMN ip text;
