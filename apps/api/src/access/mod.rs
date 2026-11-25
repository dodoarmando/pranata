//! Roles and Permissions (D-33). The catalogue of Permissions is fixed in code
//! and mirrored by a check constraint on `role_permissions`; a Role is a named
//! set of them, and the Administrator Role holds every one by definition.

pub mod roles;
pub mod users;

use std::str::FromStr;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;

use crate::{
    ApiError,
    auth::CurrentUser,
    ids::{RoleId, UserId},
};

/// One thing a User may be allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum Permission {
    #[serde(rename = "village.manage")]
    VillageManage,
    #[serde(rename = "content.write")]
    ContentWrite,
    #[serde(rename = "content.publish")]
    ContentPublish,
    #[serde(rename = "budget.manage")]
    BudgetManage,
    #[serde(rename = "audit.read")]
    AuditRead,
    #[serde(rename = "user.manage")]
    UserManage,
    #[serde(rename = "role.manage")]
    RoleManage,
}

/// A string that is not a Permission of the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPermission(pub String);

impl std::fmt::Display for UnknownPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown permission {:?}", self.0)
    }
}

impl std::error::Error for UnknownPermission {}

impl Permission {
    /// Every Permission, in the order the interface lists them.
    pub const ALL: [Self; 7] = [
        Self::VillageManage,
        Self::ContentWrite,
        Self::ContentPublish,
        Self::BudgetManage,
        Self::AuditRead,
        Self::UserManage,
        Self::RoleManage,
    ];

    /// The dotted name stored in the database and sent over the API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VillageManage => "village.manage",
            Self::ContentWrite => "content.write",
            Self::ContentPublish => "content.publish",
            Self::BudgetManage => "budget.manage",
            Self::AuditRead => "audit.read",
            Self::UserManage => "user.manage",
            Self::RoleManage => "role.manage",
        }
    }
}

impl FromStr for Permission {
    type Err = UnknownPermission;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|permission| permission.as_str() == text)
            .ok_or_else(|| UnknownPermission(text.to_owned()))
    }
}

/// The Role a User holds, as shown to clients.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoleRef {
    #[schema(value_type = uuid::Uuid)]
    pub id: RoleId,
    pub name: String,
    pub is_administrator: bool,
}

/// The Permissions a Role grants: all of them for the Administrator Role,
/// otherwise the stored names, in catalogue order.
///
/// # Errors
/// Fails when a stored name is not in the catalogue, which the check
/// constraint rules out.
pub fn granted(is_administrator: bool, stored: &[String]) -> anyhow::Result<Vec<Permission>> {
    if is_administrator {
        return Ok(Permission::ALL.to_vec());
    }
    let held = stored
        .iter()
        .map(|name| name.parse::<Permission>())
        .collect::<Result<Vec<_>, _>>()
        .context("reading role permissions")?;
    Ok(Permission::ALL
        .into_iter()
        .filter(|permission| held.contains(permission))
        .collect())
}

/// Load the Role of a User and the Permissions it grants.
///
/// # Errors
/// Fails when the User does not exist or the query fails.
pub async fn load_for_user(
    conn: &mut PgConnection,
    user: UserId,
) -> anyhow::Result<(RoleRef, Vec<Permission>)> {
    let row = sqlx::query!(
        r#"SELECT r.id AS "id: RoleId", r.name, r.is_administrator,
                  COALESCE(array_agg(rp.permission) FILTER (WHERE rp.permission IS NOT NULL), '{}')
                      AS "permissions!: Vec<String>"
           FROM users u
           JOIN roles r ON r.id = u.role_id
           LEFT JOIN role_permissions rp ON rp.role_id = r.id
           WHERE u.id = $1
           GROUP BY r.id"#,
        user.0,
    )
    .fetch_one(conn)
    .await
    .context("loading the role of a user")?;
    let permissions = granted(row.is_administrator, &row.permissions)?;
    let role = RoleRef {
        id: row.id,
        name: row.name,
        is_administrator: row.is_administrator,
    };
    Ok((role, permissions))
}

/// Load a Role and the Permissions it grants, or `None` when no Role has
/// that id.
///
/// # Errors
/// Fails when the query fails.
pub async fn load_role(
    conn: &mut PgConnection,
    id: RoleId,
) -> anyhow::Result<Option<(RoleRef, Vec<Permission>)>> {
    let Some(row) = sqlx::query!(
        r#"SELECT r.id AS "id: RoleId", r.name, r.is_administrator,
                  COALESCE(array_agg(rp.permission) FILTER (WHERE rp.permission IS NOT NULL), '{}')
                      AS "permissions!: Vec<String>"
           FROM roles r
           LEFT JOIN role_permissions rp ON rp.role_id = r.id
           WHERE r.id = $1
           GROUP BY r.id"#,
        id.0,
    )
    .fetch_optional(conn)
    .await
    .context("loading a role")?
    else {
        return Ok(None);
    };
    let permissions = granted(row.is_administrator, &row.permissions)?;
    let role = RoleRef {
        id: row.id,
        name: row.name,
        is_administrator: row.is_administrator,
    };
    Ok(Some((role, permissions)))
}

/// How far a non-Administrator's reach falls short of a target Role.
enum Shortfall {
    /// The target is the Administrator Role.
    Administrator,
    /// The target grants a Permission the actor lacks.
    Permission,
}

/// Whether `actor` may act on something holding a Role: an Administrator may
/// act on anything; anyone else not on the Administrator Role, and only on a
/// Role whose Permissions they all hold.
fn shortfall(
    actor: &CurrentUser,
    target_is_administrator: bool,
    target_permissions: &[Permission],
) -> Option<Shortfall> {
    if actor.role.is_administrator {
        None
    } else if target_is_administrator {
        Some(Shortfall::Administrator)
    } else if target_permissions
        .iter()
        .all(|permission| actor.permissions.contains(permission))
    {
        None
    } else {
        Some(Shortfall::Permission)
    }
}

/// The escalation rules (D-33) for handing out `target_role`: assigning it to
/// a User, or putting `target_permissions` into a Role. A non-Administrator
/// can never hand out more than they have.
///
/// # Errors
/// 403 `administrator_only` when the target is the Administrator Role and the
/// actor is not an Administrator; 403 `permission_not_held` when the target
/// grants a Permission the actor lacks.
pub fn can_manage(
    actor: &CurrentUser,
    target_role: &RoleRef,
    target_permissions: &[Permission],
) -> Result<(), ApiError> {
    match shortfall(actor, target_role.is_administrator, target_permissions) {
        None => Ok(()),
        Some(Shortfall::Administrator) => Err(ApiError::Forbidden("administrator_only")),
        Some(Shortfall::Permission) => Err(ApiError::Forbidden("permission_not_held")),
    }
}

/// The escalation rules (D-33) for putting `permissions` into a Role (never
/// the Administrator Role, which nobody edits).
///
/// # Errors
/// 403 `permission_not_held` when a non-Administrator lacks one of them.
pub fn can_grant(actor: &CurrentUser, permissions: &[Permission]) -> Result<(), ApiError> {
    match shortfall(actor, false, permissions) {
        None => Ok(()),
        Some(_) => Err(ApiError::Forbidden("permission_not_held")),
    }
}

/// The escalation rules (D-33) for acting on a User who holds `target_role`.
/// A non-Administrator may not act on an Administrator, nor on a User whose
/// Permissions reach beyond their own: they could take over that account
/// (with an Access Link) or strip it of what they cannot grant back.
///
/// # Errors
/// 403 `administrator_only` when the User is an Administrator and the actor
/// is not; 403 `stronger_user` when the User holds a Permission the actor
/// lacks.
pub fn can_manage_user(
    actor: &CurrentUser,
    target_role: &RoleRef,
    target_permissions: &[Permission],
) -> Result<(), ApiError> {
    match shortfall(actor, target_role.is_administrator, target_permissions) {
        None => Ok(()),
        Some(Shortfall::Administrator) => Err(ApiError::Forbidden("administrator_only")),
        Some(Shortfall::Permission) => Err(ApiError::Forbidden("stronger_user")),
    }
}

/// The id of the built-in Administrator Role.
///
/// # Errors
/// Fails when the query fails or the migrations did not create the Role.
pub async fn administrator_role(db: &PgPool) -> anyhow::Result<RoleId> {
    sqlx::query_scalar!(r#"SELECT id AS "id: RoleId" FROM roles WHERE is_administrator"#)
        .fetch_one(db)
        .await
        .context("finding the Administrator Role")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(is_administrator: bool) -> RoleRef {
        RoleRef {
            id: RoleId(uuid::Uuid::nil()),
            name: "Peran".into(),
            is_administrator,
        }
    }

    fn actor(is_administrator: bool, permissions: &[Permission]) -> CurrentUser {
        CurrentUser {
            id: UserId(uuid::Uuid::nil()),
            username: "budi".into(),
            display_name: "Budi".into(),
            role: role(is_administrator),
            permissions: permissions.to_vec(),
        }
    }

    fn code(result: &Result<(), ApiError>) -> Option<&'static str> {
        match *result {
            Ok(()) => None,
            Err(ApiError::Forbidden(code)) => Some(code),
            Err(ref other) => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn administrators_manage_everything() {
        let admin = actor(true, &Permission::ALL);
        assert_eq!(
            code(&can_manage(&admin, &role(true), &Permission::ALL)),
            None
        );
        assert_eq!(
            code(&can_manage(&admin, &role(false), &Permission::ALL)),
            None
        );
    }

    #[test]
    fn others_never_touch_the_administrator_role() {
        let manager = actor(false, &Permission::ALL);
        assert_eq!(
            code(&can_manage(&manager, &role(true), &[])),
            Some("administrator_only")
        );
    }

    #[test]
    fn others_act_only_within_their_permissions() {
        let manager = actor(false, &[Permission::UserManage, Permission::ContentWrite]);
        assert_eq!(
            code(&can_manage(
                &manager,
                &role(false),
                &[Permission::ContentWrite]
            )),
            None
        );
        assert_eq!(code(&can_manage(&manager, &role(false), &[])), None);
        assert_eq!(
            code(&can_manage(
                &manager,
                &role(false),
                &[Permission::ContentWrite, Permission::VillageManage]
            )),
            Some("permission_not_held")
        );
    }

    #[test]
    fn a_stronger_user_is_named_as_such() {
        let manager = actor(false, &[Permission::UserManage]);
        assert_eq!(
            code(&can_manage_user(
                &manager,
                &role(false),
                &[Permission::UserManage, Permission::VillageManage]
            )),
            Some("stronger_user")
        );
        assert_eq!(
            code(&can_manage_user(&manager, &role(true), &[])),
            Some("administrator_only")
        );
        assert_eq!(
            code(&can_manage_user(
                &manager,
                &role(false),
                &[Permission::UserManage]
            )),
            None
        );
    }
}
