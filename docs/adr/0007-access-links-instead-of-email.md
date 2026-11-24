# Access Links instead of email for onboarding and resets

Pranata sends no email in R1. A new User, or a User who lost access, receives a single-use, short-lived Access Link that an Administrator creates (under sudo mode) and hands over in person or by message. The first Administrator's link is created with a server CLI during installation. A registered passkey is a second way back in.

Villages rarely have reliable outgoing email, and a misconfigured mail server would make "forgot password" fail silently. Staff already coordinate through WhatsApp, so handing over a link fits how they work.

## Considered Options

- **Email reset links**: familiar, but needs SMTP credentials and deliverability per Installation.
- **Temporary passwords set by an Administrator**: the Administrator would know the User's password.

## Consequences

The login page's "Lupa kata sandi?" tells the User to contact an Administrator. Optional email can be added in R4 without changing Access Links.
