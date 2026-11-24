//! Typed identifiers shared across modules.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifier of a user. Task 6 extends this module once users exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct UserId(pub Uuid);

/// Identifier of an audit entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct AuditEntryId(pub Uuid);
