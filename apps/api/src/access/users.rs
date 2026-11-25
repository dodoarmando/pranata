//! User administration (D-21, D-32, D-33): list, create, edit, deactivate and
//! reactivate Users, issue Access Links and remove Passkeys. Users are never
//! deleted.
//!
//! Every write checks, in this order: the escalation rules
//! ([`super::can_manage`] for the Role handed out, [`super::can_manage_user`]
//! for the User acted on), request validation, then the domain rules. The
//! caller has already checked `user.manage` and Sudo Mode.
//!
//! Lock order. A transaction here that locks several rows takes them in this
//! order: the active Administrators (by id), then the target User, then the
//! target's sessions, Access Links and Passkeys. `access_link::redeem` and
//! [`crate::auth::passkey`] also lock the User before the link or the
//! Passkey, so none of these transactions deadlock with each other. Locks on
//! `users` rows are `FOR NO KEY UPDATE`, not `FOR UPDATE`: inserting a row
//! that references a User (an Access Link, a session, a Passkey, an Audit
//! Entry naming the actor) takes a key-share lock on that User, which
//! `FOR UPDATE` would block. An Administrator issuing a link while another
//! deactivates someone could then deadlock on each other's rows.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use url::Url;
use utoipa::ToSchema;

use super::{Permission, RoleRef, can_manage, can_manage_user};
use crate::{
    ApiError, FieldError,
    audit::{self, AuditAction, NewAuditEntry},
    auth::{self, CurrentUser, access_link},
    ids::{PasskeyId, RoleId, UserId},
};

/// A User as the Users screen lists them.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserSummary {
    #[schema(value_type = uuid::Uuid)]
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub role: RoleRef,
    pub is_active: bool,
    /// Whether the User has set a password, by redeeming an Access Link.
    pub has_password: bool,
    /// How many Passkeys the User registered.
    pub passkey_count: i64,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub created_at: OffsetDateTime,
}

/// A new Access Link, shown once. Only the hash of its token is stored.
#[derive(Serialize, ToSchema)]
pub struct AccessLinkIssued {
    /// `{origin}/akses#{token}`: open it to set a password.
    pub url: String,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub expires_at: OffsetDateTime,
}

impl std::fmt::Debug for AccessLinkIssued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessLinkIssued")
            .field("url", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl AccessLinkIssued {
    /// The link for `issued`, under the Installation's public origin.
    #[must_use]
    pub fn new(origin: &Url, issued: &IssuedLink) -> Self {
        Self {
            url: access_link::url(origin, &issued.token),
            expires_at: issued.expires_at,
        }
    }
}

/// A created User and their first Access Link.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedUser {
    pub user: UserSummary,
    pub access_link: AccessLinkIssued,
}

/// What an Administrator fills in to add a User.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    /// Lower-cased and trimmed; it can never change or be reused.
    pub username: String,
    pub display_name: String,
    #[schema(value_type = uuid::Uuid)]
    pub role_id: RoleId,
}

/// An edit of a User. A missing field stays as it is. The username never
/// changes, so a `username` field is rejected.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    #[schema(value_type = Option<uuid::Uuid>)]
    pub role_id: Option<RoleId>,
}

/// The token of a new Access Link and when it expires.
pub struct IssuedLink {
    pub token: String,
    pub expires_at: OffsetDateTime,
}

impl std::fmt::Debug for IssuedLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedLink")
            .field("token", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

struct SummaryRow {
    id: UserId,
    username: String,
    display_name: String,
    is_active: bool,
    has_password: bool,
    passkey_count: i64,
    created_at: OffsetDateTime,
    role_id: RoleId,
    role_name: String,
    is_administrator: bool,
}

impl From<SummaryRow> for UserSummary {
    fn from(row: SummaryRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            display_name: row.display_name,
            role: RoleRef {
                id: row.role_id,
                name: row.role_name,
                is_administrator: row.is_administrator,
            },
            is_active: row.is_active,
            has_password: row.has_password,
            passkey_count: row.passkey_count,
            created_at: row.created_at,
        }
    }
}

/// Every User, or only `only`: active ones first, then by name.
async fn summaries(
    conn: &mut PgConnection,
    only: Option<UserId>,
) -> anyhow::Result<Vec<UserSummary>> {
    let rows = sqlx::query_as!(
        SummaryRow,
        r#"SELECT u.id AS "id: UserId", u.username, u.display_name, u.is_active,
                  u.password_hash IS NOT NULL AS "has_password!",
                  (SELECT count(*) FROM passkeys p WHERE p.user_id = u.id) AS "passkey_count!",
                  u.created_at,
                  r.id AS "role_id: RoleId", r.name AS role_name, r.is_administrator
           FROM users u JOIN roles r ON r.id = u.role_id
           WHERE $1::uuid IS NULL OR u.id = $1
           ORDER BY u.is_active DESC, lower(u.display_name), u.username"#,
        only.map(|id| id.0),
    )
    .fetch_all(conn)
    .await
    .context("listing users")?;
    Ok(rows.into_iter().map(UserSummary::from).collect())
}

async fn summary(conn: &mut PgConnection, id: UserId) -> anyhow::Result<UserSummary> {
    summaries(conn, Some(id))
        .await?
        .pop()
        .context("the user just written is missing")
}

/// Every User, active ones first, then by display name.
///
/// # Errors
/// Fails when the query fails.
pub async fn list(db: &PgPool) -> anyhow::Result<Vec<UserSummary>> {
    summaries(&mut *db.acquire().await?, None).await
}

/// The User a write acts on, locked for the rest of the transaction.
struct Target {
    id: UserId,
    username: String,
    display_name: String,
    is_active: bool,
    role: RoleRef,
    permissions: Vec<Permission>,
}

impl Target {
    fn check_actor(&self, actor: &CurrentUser) -> Result<(), ApiError> {
        can_manage_user(actor, &self.role, &self.permissions)
    }
}

async fn lock_target(conn: &mut PgConnection, id: UserId) -> Result<Option<Target>, ApiError> {
    let Some(row) = sqlx::query!(
        "SELECT username, display_name, is_active FROM users WHERE id = $1 FOR NO KEY UPDATE",
        id.0,
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let (role, permissions) = super::load_for_user(conn, id).await?;
    Ok(Some(Target {
        id,
        username: row.username,
        display_name: row.display_name,
        is_active: row.is_active,
        role,
        permissions,
    }))
}

/// An active Administrator, locked so they cannot be demoted or deactivated
/// until the transaction ends.
#[derive(Debug, Clone, Copy)]
struct LockedAdministrator {
    id: UserId,
    /// Whether they can sign in.
    has_credential: bool,
}

/// Lock every active Administrator. `ORDER BY id` makes concurrent requests
/// take the locks in the same order, so they wait instead of deadlocking. A
/// request that waited sees the rows as the other one left them: someone it
/// demoted or deactivated is no longer in the list.
///
/// A password or a Passkey is a credential. Whether they have one is read by
/// a second statement: a statement that waited for a row lock re-reads the
/// locked row, but a subquery on `passkeys` would still see the table as it
/// was before the wait. Every write that adds or removes a Passkey locks its
/// User first, so once the locks are held the answer stays true.
async fn lock_active_administrators(
    conn: &mut PgConnection,
) -> anyhow::Result<Vec<LockedAdministrator>> {
    let ids = sqlx::query_scalar!(
        "SELECT id FROM users
         WHERE is_active AND role_id = (SELECT id FROM roles WHERE is_administrator)
         ORDER BY id
         FOR NO KEY UPDATE",
    )
    .fetch_all(&mut *conn)
    .await
    .context("locking the active administrators")?;
    sqlx::query_as!(
        LockedAdministrator,
        r#"SELECT u.id AS "id: UserId",
                  (u.password_hash IS NOT NULL
                   OR EXISTS (SELECT 1 FROM passkeys p WHERE p.user_id = u.id)) AS "has_credential!"
           FROM users u
           WHERE u.id = ANY($1)
           ORDER BY u.id"#,
        &ids,
    )
    .fetch_all(conn)
    .await
    .context("reading the credentials of the active administrators")
}

/// How many of the locked Administrators other than `target` can sign in. An
/// Administrator who has not redeemed their first Access Link cannot, so they
/// do not keep the Installation reachable.
fn other_credentialed_administrators(locked: &[LockedAdministrator], target: UserId) -> usize {
    locked
        .iter()
        .filter(|administrator| administrator.id != target && administrator.has_credential)
        .count()
}

/// D-33: at least one active Administrator who can sign in must remain once
/// `target` stops being one.
fn keep_an_administrator(locked: &[LockedAdministrator], target: UserId) -> Result<(), ApiError> {
    if other_credentialed_administrators(locked, target) == 0 {
        Err(ApiError::Conflict("last_administrator"))
    } else {
        Ok(())
    }
}

/// D-33 for deleting Passkeys of `target`: one (`deleting`) or, with `None`,
/// all of them. Refuse when that leaves an active Administrator with no
/// credential at all while no other active Administrator can sign in. The
/// caller holds the locks of [`lock_active_administrators`] and the target.
async fn keep_a_credential(
    conn: &mut PgConnection,
    administrators: &[LockedAdministrator],
    target: &Target,
    deleting: Option<PasskeyId>,
) -> Result<(), ApiError> {
    if !(target.is_active && target.role.is_administrator) {
        return Ok(());
    }
    let row = sqlx::query!(
        r#"SELECT u.password_hash IS NOT NULL AS "has_password!",
                  count(p.id) AS "passkeys!",
                  count(p.id) FILTER (WHERE $2::uuid IS NULL OR p.id = $2) AS "deleting!"
           FROM users u LEFT JOIN passkeys p ON p.user_id = u.id
           WHERE u.id = $1
           GROUP BY u.id"#,
        target.id.0,
        deleting.map(|id| id.0),
    )
    .fetch_one(conn)
    .await?;
    if !row.has_password && row.deleting > 0 && row.deleting == row.passkeys {
        keep_an_administrator(administrators, target.id)
    } else {
        Ok(())
    }
}

/// Before the signed-in User deletes their own Passkey `passkey`: take the
/// locks in order (the active Administrators, then the User) and apply the
/// last-Administrator rule.
///
/// # Errors
/// 401 `unauthenticated` when the User is gone; 409 `last_administrator`.
pub(crate) async fn guard_own_passkey_removal(
    conn: &mut PgConnection,
    user: UserId,
    passkey: PasskeyId,
) -> Result<(), ApiError> {
    let administrators = lock_active_administrators(conn).await?;
    let target = lock_target(conn, user)
        .await?
        .ok_or(ApiError::Unauthorized("unauthenticated"))?;
    keep_a_credential(conn, &administrators, &target, Some(passkey)).await
}

fn not_found() -> ApiError {
    ApiError::NotFound("user_not_found")
}

fn role_not_found() -> FieldError {
    FieldError {
        field: "role_id".to_owned(),
        code: "role_not_found".to_owned(),
    }
}

/// A write that names a Role deleted after the Role was read fails on the
/// foreign key of `users.role_id`: answer as if the Role never existed.
fn role_vanished(error: ApiError) -> ApiError {
    let vanished = matches!(&error, ApiError::Internal(error) if error
    .downcast_ref::<sqlx::Error>()
    .and_then(sqlx::Error::as_database_error)
    .is_some_and(|error| {
        error.is_foreign_key_violation() && error.constraint() == Some("users_role_id_fkey")
    }));
    if vanished {
        ApiError::Validation(vec![role_not_found()])
    } else {
        error
    }
}

/// Create a User with no password and their first Access Link.
///
/// # Errors
/// 403 `administrator_only` or `permission_not_held` for a Role the actor may
/// not assign; 422 `validation_failed` (fields `username`, `display_name`,
/// `role_id`); 409 `username_taken`.
pub async fn create(
    db: &PgPool,
    actor: &CurrentUser,
    request: CreateUserRequest,
    now: OffsetDateTime,
) -> Result<(UserSummary, IssuedLink), ApiError> {
    let role = super::load_role(&mut *db.acquire().await?, request.role_id).await?;
    if let Some((role, permissions)) = &role {
        can_manage(actor, role, permissions)?;
    }

    let mut errors = Vec::new();
    if auth::normalise_username(&request.username).is_err() {
        errors.push(FieldError {
            field: "username".to_owned(),
            code: "username_invalid".to_owned(),
        });
    }
    if request.display_name.trim().is_empty() {
        errors.push(FieldError {
            field: "display_name".to_owned(),
            code: "required".to_owned(),
        });
    }
    if role.is_none() {
        errors.push(role_not_found());
    }
    if !errors.is_empty() {
        return Err(ApiError::Validation(errors));
    }

    let created = auth::create_user_with_access_link(
        db,
        &request.username,
        &request.display_name,
        request.role_id,
        Some(actor.id),
        now,
    )
    .await
    .map_err(role_vanished)?;
    let user = summary(&mut *db.acquire().await?, created.id).await?;
    let link = IssuedLink {
        token: created.access_link_token,
        expires_at: now + access_link::LIFETIME,
    };
    Ok((user, link))
}

/// The audited fields of a User.
#[derive(Serialize)]
struct Audited<'a> {
    display_name: &'a str,
    role: &'a str,
}

/// Change a User's display name or Role.
///
/// # Errors
/// 403 `administrator_only`, `permission_not_held` (the new Role),
/// `stronger_user` (the User) or `own_role`; 422
/// `validation_failed` (fields `display_name`, `role_id`); 404
/// `user_not_found`; 409 `last_administrator`.
pub async fn update(
    db: &PgPool,
    actor: &CurrentUser,
    id: UserId,
    request: UpdateUserRequest,
) -> Result<UserSummary, ApiError> {
    let mut tx = db.begin().await?;
    // A Role change may demote an Administrator, so lock them first.
    let administrators = if request.role_id.is_some() {
        lock_active_administrators(&mut tx).await?
    } else {
        Vec::new()
    };
    let target = lock_target(&mut tx, id).await?;
    let new_role = match request.role_id {
        Some(role_id) => Some(super::load_role(&mut tx, role_id).await?),
        None => None,
    };
    // The Role being handed out first: assigning the Administrator Role says
    // `administrator_only` whoever the target is.
    if let Some(Some((role, permissions))) = &new_role {
        can_manage(actor, role, permissions)?;
    }
    if let Some(target) = &target {
        target.check_actor(actor)?;
    }
    if new_role.is_some() && id == actor.id {
        return Err(ApiError::Forbidden("own_role"));
    }

    let display_name = request.display_name.as_deref().map(str::trim);
    let mut errors = Vec::new();
    if display_name.is_some_and(str::is_empty) {
        errors.push(FieldError {
            field: "display_name".to_owned(),
            code: "required".to_owned(),
        });
    }
    if matches!(new_role, Some(None)) {
        errors.push(role_not_found());
    }
    if !errors.is_empty() {
        return Err(ApiError::Validation(errors));
    }

    let target = target.ok_or_else(not_found)?;
    let role = match new_role {
        Some(Some((role, _))) => role,
        _ => target.role.clone(),
    };
    if target.is_active && target.role.is_administrator && !role.is_administrator {
        keep_an_administrator(&administrators, id)?;
    }

    let display_name = display_name.unwrap_or(&target.display_name);
    // Decide on the id, not the name: the two Role names were read by
    // different statements, and a rename in between could make a real change
    // look like none. The names only describe the change in the Audit Entry.
    if display_name != target.display_name || role.id != target.role.id {
        let changes = audit::diff(
            &Audited {
                display_name: &target.display_name,
                role: &target.role.name,
            },
            &Audited {
                display_name,
                role: &role.name,
            },
        );
        sqlx::query!(
            "UPDATE users SET display_name = $2, role_id = $3 WHERE id = $1",
            id.0,
            display_name,
            role.id.0,
        )
        .execute(&mut *tx)
        .await
        .map_err(|error| role_vanished(error.into()))?;
        audit::record(
            &mut tx,
            NewAuditEntry {
                actor: Some(actor.id),
                action: AuditAction::Update,
                object_type: "user",
                object_id: id.0.to_string(),
                summary: format!("Edited user {}", target.username),
                changes,
            },
        )
        .await?;
    }
    let user = summary(&mut tx, id).await?;
    tx.commit().await?;
    Ok(user)
}

/// Deactivate a User: end their sessions and revoke their unused Access Links.
/// Deactivating an inactive User changes nothing.
///
/// # Errors
/// 403 `administrator_only` or `stronger_user`; 404 `user_not_found`;
/// 409 `cannot_deactivate_self` or `last_administrator`.
pub async fn deactivate(
    db: &PgPool,
    actor: &CurrentUser,
    id: UserId,
    now: OffsetDateTime,
) -> Result<UserSummary, ApiError> {
    let mut tx = db.begin().await?;
    let administrators = lock_active_administrators(&mut tx).await?;
    let target = lock_target(&mut tx, id).await?.ok_or_else(not_found)?;
    target.check_actor(actor)?;
    if target.id == actor.id {
        return Err(ApiError::Conflict("cannot_deactivate_self"));
    }
    if target.is_active {
        if target.role.is_administrator {
            keep_an_administrator(&administrators, id)?;
        }
        sqlx::query!("UPDATE users SET is_active = false WHERE id = $1", id.0)
            .execute(&mut *tx)
            .await?;
        sqlx::query!("DELETE FROM sessions WHERE user_id = $1", id.0)
            .execute(&mut *tx)
            .await?;
        access_link::revoke_unused(&mut tx, id, now).await?;
        audit::record(
            &mut tx,
            NewAuditEntry {
                actor: Some(actor.id),
                action: AuditAction::Deactivate,
                object_type: "user",
                object_id: id.0.to_string(),
                summary: format!("Deactivated user {}", target.username),
                changes: Vec::new(),
            },
        )
        .await?;
    }
    let user = summary(&mut tx, id).await?;
    tx.commit().await?;
    Ok(user)
}

/// Reactivate a User. They keep their password (and Passkeys), so they can
/// sign in at once. Reactivating an active User changes nothing.
///
/// # Errors
/// 403 `administrator_only` or `stronger_user`; 404 `user_not_found`.
pub async fn reactivate(
    db: &PgPool,
    actor: &CurrentUser,
    id: UserId,
) -> Result<UserSummary, ApiError> {
    let mut tx = db.begin().await?;
    let target = lock_target(&mut tx, id).await?.ok_or_else(not_found)?;
    target.check_actor(actor)?;
    if !target.is_active {
        sqlx::query!("UPDATE users SET is_active = true WHERE id = $1", id.0)
            .execute(&mut *tx)
            .await?;
        // Deactivation deleted the sessions, but a sign-in that raced it may
        // have left one behind; it must not come back to life.
        sqlx::query!("DELETE FROM sessions WHERE user_id = $1", id.0)
            .execute(&mut *tx)
            .await?;
        audit::record(
            &mut tx,
            NewAuditEntry {
                actor: Some(actor.id),
                action: AuditAction::Reactivate,
                object_type: "user",
                object_id: id.0.to_string(),
                summary: format!("Reactivated user {}", target.username),
                changes: Vec::new(),
            },
        )
        .await?;
    }
    let user = summary(&mut tx, id).await?;
    tx.commit().await?;
    Ok(user)
}

/// Issue a new Access Link for a User, revoking their earlier unused ones.
///
/// # Errors
/// 403 `administrator_only` or `stronger_user`; 404 `user_not_found`;
/// 409 `user_inactive`.
pub async fn issue_access_link(
    db: &PgPool,
    actor: &CurrentUser,
    id: UserId,
    now: OffsetDateTime,
) -> Result<IssuedLink, ApiError> {
    let mut tx = db.begin().await?;
    let target = lock_target(&mut tx, id).await?.ok_or_else(not_found)?;
    target.check_actor(actor)?;
    if !target.is_active {
        return Err(ApiError::Conflict("user_inactive"));
    }
    let token = access_link::create(&mut tx, id, Some(actor.id), now).await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(actor.id),
            action: AuditAction::IssueAccessLink,
            object_type: "user",
            object_id: id.0.to_string(),
            summary: format!("Issued an Access Link for {}", target.username),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(IssuedLink {
        token,
        expires_at: now + access_link::LIFETIME,
    })
}

/// Remove every Passkey of a User, as recovery after an account takeover.
///
/// # Errors
/// 403 `administrator_only` or `stronger_user`; 404 `user_not_found`; 409
/// `last_administrator`.
pub async fn remove_passkeys(db: &PgPool, actor: &CurrentUser, id: UserId) -> Result<(), ApiError> {
    let mut tx = db.begin().await?;
    let administrators = lock_active_administrators(&mut tx).await?;
    let target = lock_target(&mut tx, id).await?.ok_or_else(not_found)?;
    target.check_actor(actor)?;
    keep_a_credential(&mut tx, &administrators, &target, None).await?;
    let removed = sqlx::query!("DELETE FROM passkeys WHERE user_id = $1", id.0)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if removed > 0 {
        audit::record(
            &mut tx,
            NewAuditEntry {
                actor: Some(actor.id),
                action: AuditAction::Delete,
                object_type: "passkey",
                object_id: id.0.to_string(),
                summary: format!("Removed all passkeys of {}", target.username),
                changes: Vec::new(),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> UserId {
        UserId(uuid::Uuid::from_u128(n))
    }

    fn admin(n: u128, has_credential: bool) -> LockedAdministrator {
        LockedAdministrator {
            id: id(n),
            has_credential,
        }
    }

    #[test]
    fn counts_other_administrators_who_can_sign_in() {
        let locked = [admin(1, true), admin(2, true), admin(3, false)];
        assert_eq!(other_credentialed_administrators(&locked, id(1)), 1);
        assert_eq!(other_credentialed_administrators(&locked, id(3)), 2);
        // A target that is not among them leaves every credentialed one.
        assert_eq!(other_credentialed_administrators(&locked, id(9)), 2);
    }

    #[test]
    fn uncredentialed_administrators_do_not_count() {
        let locked = [admin(1, true), admin(2, false)];
        assert_eq!(other_credentialed_administrators(&locked, id(1)), 0);
        assert!(matches!(
            keep_an_administrator(&locked, id(1)),
            Err(ApiError::Conflict("last_administrator"))
        ));
        assert!(keep_an_administrator(&locked, id(2)).is_ok());
    }

    #[test]
    fn nobody_left_is_refused() {
        assert_eq!(other_credentialed_administrators(&[], id(1)), 0);
        assert!(keep_an_administrator(&[admin(1, true)], id(1)).is_err());
    }
}
