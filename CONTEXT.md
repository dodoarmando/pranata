# Pranata

Pranata is a village website and administration product sold to many Indonesian villages: a public site for residents plus an admin area where village staff run the village's records, letters, and site content.

## Language

### Deployment

**Installation**:
One running deployment of Pranata, serving exactly one Village with its own database.
_Avoid_: Tenant, instance, site

**Village**:
The single Indonesian village (desa) an Installation belongs to and publishes for.
_Avoid_: Desa (in code), tenant, client, organization

### Access

**Permission**:
One action the system allows or denies, drawn from a fixed catalog defined by Pranata itself.
_Avoid_: Right, privilege, hak akses (in code)

**Role**:
A named set of Permissions that a Village defines for itself.
_Avoid_: Group, peran (in code)

**Administrator**:
The built-in Role that always holds every Permission and alone manages Users and Roles.
_Avoid_: Admin, superuser, root

**User**:
A person who signs in to the admin area on behalf of the Village, holding exactly one Role; never deleted, only deactivated. Residents are not Users.
_Avoid_: Account, staff, operator, pengguna (in code)

**Audit Entry**:
One immutable record of who did what to which object, when, and what changed.
_Avoid_: Activity, history, event

**Audit Log**:
The complete, append-only collection of Audit Entries for an Installation.
_Avoid_: Activity log, history
