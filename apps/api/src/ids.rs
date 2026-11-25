//! Typed identifiers shared across modules.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifier of a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct UserId(pub Uuid);

/// Identifier of an audit entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct AuditEntryId(pub Uuid);

/// Identifier of a role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct RoleId(pub Uuid);
