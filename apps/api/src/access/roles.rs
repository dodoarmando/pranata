//! Role administration (D-33): list, create, edit and delete Roles built from
//! the fixed catalogue of Permissions. The Administrator Role is built in and
//! read-only; a Role that any User holds, active or not, cannot be deleted.
//!
//! Every write checks, in this order: the escalation rules, request
//! validation, then the domain rules. The caller has already checked
//! `role.manage` and Sudo Mode.
//!
//! Locks. An edit locks the Role row `FOR NO KEY UPDATE`, so concurrent edits
//! of one Role apply one after the other while Users can still be given it. A
//! delete locks it `FOR UPDATE`, which waits for any transaction that is
//! giving the Role to a User (that insert or update holds a key-share lock on
//! the row) and holds off new ones; the "in use" check after the lock then
//! sees every User who holds the Role.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;

use super::{Permission, RoleRef, can_grant, can_manage, granted};
use crate::{
    ApiError, FieldError,
    audit::{self, AuditAction, NewAuditEntry},
    auth::CurrentUser,
    ids::RoleId,
};

/// The longest Role name, in characters.
const NAME_MAX: usize = 50;

/// A Role as the Roles screen lists it.
#[derive(Debug, Serialize, ToSchema)]
pub struct RoleDetail {
    #[schema(value_type = uuid::Uuid)]
    pub id: RoleId,
    pub name: String,
    pub is_administrator: bool,
    /// What the Role grants, in catalogue order: every Permission for the
    /// Administrator Role.
    pub permissions: Vec<Permission>,
    /// How many active Users hold the Role.
    pub user_count: i64,
}

/// What an editor fills in to create or edit a Role.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RoleRequest {
    /// Trimmed; 1 to 50 characters, unique ignoring case.
    pub name: String,
    /// A set: duplicates collapse.
    pub permissions: Vec<Permission>,
}

impl RoleRequest {
    /// The trimmed name and the Permissions in catalogue order, or the field
    /// errors.
    fn validate(&self) -> Result<(&str, Vec<Permission>), ApiError> {
        let name = self.name.trim();
        if !(1..=NAME_MAX).contains(&name.chars().count()) {
            return Err(ApiError::Validation(vec![FieldError {
                field: "name".to_owned(),
                code: "name_length".to_owned(),
            }]));
        }
        Ok((name, catalogue_order(&self.permissions)))
    }
}

/// `permissions` without duplicates, in catalogue order.
fn catalogue_order(permissions: &[Permission]) -> Vec<Permission> {
    Permission::ALL
        .into_iter()
        .filter(|permission| permissions.contains(permission))
        .collect()
}

/// Every Role, or only `only`: the Administrator Role first, then by name.
async fn details(conn: &mut PgConnection, only: Option<RoleId>) -> anyhow::Result<Vec<RoleDetail>> {
    let rows = sqlx::query!(
        r#"SELECT r.id AS "id: RoleId", r.name, r.is_administrator,
                  COALESCE((SELECT array_agg(rp.permission) FROM role_permissions rp
                            WHERE rp.role_id = r.id), '{}') AS "permissions!: Vec<String>",
                  (SELECT count(*) FROM users u WHERE u.role_id = r.id AND u.is_active)
                      AS "user_count!"
           FROM roles r
           WHERE $1::uuid IS NULL OR r.id = $1
           ORDER BY r.is_administrator DESC, lower(r.name), r.name"#,
        only.map(|id| id.0),
    )
    .fetch_all(conn)
    .await
    .context("listing roles")?;
    rows.into_iter()
        .map(|row| {
            Ok(RoleDetail {
                permissions: granted(row.is_administrator, &row.permissions)?,
                id: row.id,
                name: row.name,
                is_administrator: row.is_administrator,
                user_count: row.user_count,
            })
        })
        .collect()
}

async fn detail(conn: &mut PgConnection, id: RoleId) -> anyhow::Result<RoleDetail> {
    details(conn, Some(id))
        .await?
        .pop()
        .context("the role just written is missing")
}

/// Every Role: the Administrator Role first, then the others by name.
///
/// # Errors
/// Fails when the query fails.
pub async fn list(db: &PgPool) -> anyhow::Result<Vec<RoleDetail>> {
    details(&mut *db.acquire().await?, None).await
}

/// The Role a write acts on, locked for the rest of the transaction.
struct Target {
    role: RoleRef,
    permissions: Vec<Permission>,
}

impl Target {
    /// The escalation rules for changing or deleting this Role: a
    /// non-Administrator may not touch the Role they hold, nor a Role that
    /// grants a Permission they lack (or the Administrator Role).
    fn check_actor(&self, actor: &CurrentUser) -> Result<(), ApiError> {
        if !actor.role.is_administrator && actor.role.id == self.role.id {
            return Err(ApiError::Forbidden("own_role"));
        }
        can_manage(actor, &self.role, &self.permissions)
    }

    /// D-33: the Administrator Role is built in.
    fn check_not_builtin(&self) -> Result<(), ApiError> {
        if self.role.is_administrator {
            Err(ApiError::Conflict("role_builtin"))
        } else {
            Ok(())
        }
    }
}

/// How [`lock_target`] locks the Role row.
#[derive(Clone, Copy)]
enum Lock {
    /// For an edit: Users can still be given the Role meanwhile.
    Edit,
    /// For a delete: waits for, and then holds off, Users being given it.
    Delete,
}

async fn lock_target(
    conn: &mut PgConnection,
    id: RoleId,
    lock: Lock,
) -> Result<Option<Target>, ApiError> {
    let found = match lock {
        Lock::Edit => sqlx::query_scalar!(
            "SELECT 1 AS found FROM roles WHERE id = $1 FOR NO KEY UPDATE",
            id.0
        )
        .fetch_optional(&mut *conn)
        .await?
        .is_some(),
        Lock::Delete => sqlx::query_scalar!(
            "SELECT 1 AS found FROM roles WHERE id = $1 FOR UPDATE",
            id.0
        )
        .fetch_optional(&mut *conn)
        .await?
        .is_some(),
    };
    if !found {
        return Ok(None);
    }
    let (role, permissions) = super::load_role(conn, id)
        .await?
        .context("the locked role is missing")?;
    Ok(Some(Target { role, permissions }))
}

fn not_found() -> ApiError {
    ApiError::NotFound("role_not_found")
}

/// Another Role already has the name, ignoring case.
fn name_taken(error: sqlx::Error) -> ApiError {
    let taken = error.as_database_error().is_some_and(|error| {
        error.is_unique_violation() && error.constraint() == Some("roles_name_key")
    });
    if taken {
        ApiError::Conflict("role_name_taken")
    } else {
        error.into()
    }
}

async fn insert_permissions(
    conn: &mut PgConnection,
    id: RoleId,
    permissions: &[Permission],
) -> Result<(), ApiError> {
    let names: Vec<String> = permissions
        .iter()
        .map(|permission| permission.as_str().to_owned())
        .collect();
    sqlx::query!(
        "INSERT INTO role_permissions (role_id, permission) SELECT $1, unnest($2::text[])",
        id.0,
        &names,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Create a Role.
///
/// # Errors
/// 403 `permission_not_held`; 422 `validation_failed` (field `name`, code
/// `name_length`); 409 `role_name_taken`.
pub async fn create(
    db: &PgPool,
    actor: &CurrentUser,
    request: &RoleRequest,
) -> Result<RoleDetail, ApiError> {
    can_grant(actor, &request.permissions)?;
    let (name, permissions) = request.validate()?;

    let mut tx = db.begin().await?;
    let id = sqlx::query_scalar!(
        r#"INSERT INTO roles (name) VALUES ($1) RETURNING id AS "id: RoleId""#,
        name,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(name_taken)?;
    insert_permissions(&mut tx, id, &permissions).await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(actor.id),
            action: AuditAction::Create,
            object_type: "role",
            object_id: id.0.to_string(),
            summary: format!("Created role {name}"),
            changes: Vec::new(),
        },
    )
    .await?;
    let role = detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(role)
}

/// The audited fields of a Role.
#[derive(Serialize)]
struct Audited<'a> {
    name: &'a str,
    permissions: &'a [Permission],
}

/// Rename a Role or change its Permissions. Its holders, signed in or not,
/// get the new Permissions on their next request.
///
/// # Errors
/// 403 `own_role`, `administrator_only` or `permission_not_held`; 422
/// `validation_failed` (field `name`, code `name_length`); 404
/// `role_not_found`; 409 `role_builtin` or `role_name_taken`.
pub async fn update(
    db: &PgPool,
    actor: &CurrentUser,
    id: RoleId,
    request: &RoleRequest,
) -> Result<RoleDetail, ApiError> {
    let mut tx = db.begin().await?;
    let target = lock_target(&mut tx, id, Lock::Edit).await?;
    if let Some(target) = &target {
        target.check_actor(actor)?;
    }
    can_grant(actor, &request.permissions)?;
    let (name, permissions) = request.validate()?;

    let target = target.ok_or_else(not_found)?;
    target.check_not_builtin()?;
    let changes = audit::diff(
        &Audited {
            name: &target.role.name,
            permissions: &target.permissions,
        },
        &Audited {
            name,
            permissions: &permissions,
        },
    );
    if !changes.is_empty() {
        sqlx::query!("UPDATE roles SET name = $2 WHERE id = $1", id.0, name)
            .execute(&mut *tx)
            .await
            .map_err(name_taken)?;
        sqlx::query!("DELETE FROM role_permissions WHERE role_id = $1", id.0)
            .execute(&mut *tx)
            .await?;
        insert_permissions(&mut tx, id, &permissions).await?;
        audit::record(
            &mut tx,
            NewAuditEntry {
                actor: Some(actor.id),
                action: AuditAction::Update,
                object_type: "role",
                object_id: id.0.to_string(),
                summary: format!("Edited role {name}"),
                changes,
            },
        )
        .await?;
    }
    let role = detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(role)
}

/// Delete a Role that no User holds.
///
/// # Errors
/// 403 `own_role`, `administrator_only` or `permission_not_held`; 404
/// `role_not_found`; 409 `role_builtin` or `role_in_use`.
pub async fn delete(db: &PgPool, actor: &CurrentUser, id: RoleId) -> Result<(), ApiError> {
    let mut tx = db.begin().await?;
    let target = lock_target(&mut tx, id, Lock::Delete)
        .await?
        .ok_or_else(not_found)?;
    target.check_actor(actor)?;
    target.check_not_builtin()?;
    // Deactivated Users count: they keep their Role and may come back.
    let in_use = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM users WHERE role_id = $1) AS "in_use!""#,
        id.0
    )
    .fetch_one(&mut *tx)
    .await?;
    if in_use {
        return Err(ApiError::Conflict("role_in_use"));
    }
    sqlx::query!("DELETE FROM roles WHERE id = $1", id.0)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(actor.id),
            action: AuditAction::Delete,
            object_type: "role",
            object_id: id.0.to_string(),
            summary: format!("Deleted role {}", target.role.name),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(name: &str, permissions: &[Permission]) -> RoleRequest {
        RoleRequest {
            name: name.to_owned(),
            permissions: permissions.to_vec(),
        }
    }

    #[test]
    fn names_are_trimmed_and_counted_in_characters() {
        let ok = request("  Operator  ", &[]);
        assert_eq!(ok.validate().unwrap().0, "Operator");
        assert!(request(&"é".repeat(50), &[]).validate().is_ok());
        for name in ["", "   ", &"x".repeat(51)] {
            assert!(matches!(
                request(name, &[]).validate(),
                Err(ApiError::Validation(_))
            ));
        }
    }

    #[test]
    fn permissions_collapse_into_catalogue_order() {
        let input = request(
            "Peran",
            &[
                Permission::RoleManage,
                Permission::VillageManage,
                Permission::RoleManage,
            ],
        );
        assert_eq!(
            input.validate().unwrap().1,
            [Permission::VillageManage, Permission::RoleManage]
        );
    }
}
