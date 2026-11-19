use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

/// Who an Audit Entry says acted.
#[derive(Clone, Copy)]
pub enum Actor {
    /// A signed-in User.
    User(Uuid),
    /// The `pranata` CLI, run by the vendor.
    System,
    /// Someone not (yet) identified, such as a failed sign-in.
    Visitor,
}

/// Appends an Audit Entry; call it in the transaction of the change it records.
///
/// # Errors
/// When the insert fails, which must abort the change.
pub async fn record(
    conn: &mut PgConnection,
    actor: Actor,
    action: &str,
    (object_type, object_id): (&str, Option<&str>),
    details: Value,
) -> sqlx::Result<()> {
    let (kind, actor_id) = match actor {
        Actor::User(id) => ("user", Some(id)),
        Actor::System => ("system", None),
        Actor::Visitor => ("visitor", None),
    };
    sqlx::query!(
        "INSERT INTO audit_entries (actor, actor_id, action, object_type, object_id, details)
         VALUES ($1, $2, $3, $4, $5, $6)",
        kind,
        actor_id,
        action,
        object_type,
        object_id,
        details,
    )
    .execute(conn)
    .await?;
    Ok(())
}
