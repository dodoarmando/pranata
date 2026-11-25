//! Roles and Permissions (D-33). The catalogue of Permissions is fixed in code
//! and mirrored by a check constraint on `role_permissions`; a Role is a named
//! set of them, and the Administrator Role holds every one by definition.

use std::str::FromStr;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;

use crate::ids::{RoleId, UserId};

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
