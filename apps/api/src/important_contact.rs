//! Important Contacts: phone numbers residents may need, grouped by the Village's ordered
//! contact categories, saved as Drafts and then Published.
//!
//! `content.write` creates and edits Drafts; `content.publish` also publishes, unpublishes,
//! edits Published contacts, deletes Drafts and manages the contact categories.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState,
    audit::{self, Actor},
    auth::Session,
    category::{Category, CategoryFields, ensure_publisher, unique_name},
    government::Created,
    news_post::{ensure_writer, saved},
    village::diff,
};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_categories, create_category))
        .routes(routes!(update_category, delete_category))
        .routes(routes!(order_categories))
        .routes(routes!(list, create))
        .routes(routes!(update, delete))
        .routes(routes!(publish))
        .routes(routes!(unpublish))
        .routes(routes!(list_published))
}

/// A new manual order: every item of the list exactly once, first to last.
#[derive(Deserialize, ToSchema)]
pub struct Order {
    pub(crate) ids: Vec<Uuid>,
}

impl Order {
    /// Whether `ids` holds exactly the items of `old`.
    pub(crate) fn matches(&self, old: &[Uuid]) -> bool {
        let (mut old, mut new) = (old.to_vec(), self.ids.clone());
        old.sort_unstable();
        new.sort_unstable();
        old == new
    }
}

/// Every contact category in order, for the public site and the admin area.
#[utoipa::path(
    get,
    path = "/contact-categories",
    operation_id = "listContactCategories",
    tag = "content",
    responses((status = OK, description = "Every contact category", body = Vec<Category>)),
)]
pub(crate) async fn list_categories(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Category>>, ApiError> {
    let categories = sqlx::query_as!(
        Category,
        "SELECT id, name FROM contact_categories ORDER BY sort_order, id"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(categories))
}

/// Adds a contact category, last.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `409` for a name in use,
/// `422` for a blank name or one over 50 characters.
#[utoipa::path(
    post,
    path = "/contact-categories",
    operation_id = "createContactCategory",
    tag = "content",
    request_body = CategoryFields,
    responses(
        (status = CREATED, description = "The new contact category", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = CONFLICT, description = "Another contact category has the name"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn create_category(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<CategoryFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_publisher(&session)?;
    let name = body.name()?;
    let mut tx = pool.begin().await?;
    let id = unique_name(
        sqlx::query_scalar!(
            "INSERT INTO contact_categories (name, sort_order)
             VALUES ($1, (SELECT coalesce(max(sort_order), 0) + 1 FROM contact_categories))
             RETURNING id",
            name
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "contact_category.create",
        ("contact_category", Some(&id.to_string())),
        json!([{ "field": "name", "from": null, "to": name }]),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Renames a contact category.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// contact category, `409` for a name in use, `422` for a blank name or one over 50
/// characters.
#[utoipa::path(
    put,
    path = "/contact-categories/{id}",
    operation_id = "updateContactCategory",
    tag = "content",
    params(("id" = Uuid, Path, description = "The contact category")),
    request_body = CategoryFields,
    responses(
        (status = NO_CONTENT, description = "The contact category is renamed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such contact category"),
        (status = CONFLICT, description = "Another contact category has the name"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn update_category(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<CategoryFields>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let name = body.name()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query_scalar!(
        "SELECT name FROM contact_categories WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old != name {
        unique_name(
            sqlx::query!(
                "UPDATE contact_categories SET name = $1 WHERE id = $2",
                name,
                id
            )
            .execute(&mut *tx)
            .await,
        )?;
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "contact_category.update",
            ("contact_category", Some(&id.to_string())),
            json!([{ "field": "name", "from": old, "to": name }]),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a contact category no contact, Draft or Published, is filed under.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// contact category, `409` while a contact is filed under it.
#[utoipa::path(
    delete,
    path = "/contact-categories/{id}",
    operation_id = "deleteContactCategory",
    tag = "content",
    params(("id" = Uuid, Path, description = "The contact category")),
    responses(
        (status = NO_CONTENT, description = "The contact category is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such contact category"),
        (status = CONFLICT, description = "A contact is filed under the contact category"),
    ),
)]
pub(crate) async fn delete_category(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let name = match sqlx::query_scalar!(
        "DELETE FROM contact_categories WHERE id = $1 RETURNING name",
        id
    )
    .fetch_optional(&mut *tx)
    .await
    {
        Err(sqlx::Error::Database(error)) if error.is_foreign_key_violation() => {
            return Err(ApiError::Status(StatusCode::CONFLICT));
        }
        name => name?.ok_or(ApiError::Status(StatusCode::NOT_FOUND))?,
    };
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "contact_category.delete",
        ("contact_category", Some(&id.to_string())),
        json!([{ "field": "name", "from": name, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts every contact category in a new order.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `422` unless the list holds
/// every contact category exactly once.
#[utoipa::path(
    put,
    path = "/contact-categories/order",
    operation_id = "orderContactCategories",
    tag = "content",
    request_body = Order,
    responses(
        (status = NO_CONTENT, description = "The contact categories are reordered"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = UNPROCESSABLE_ENTITY, description = "The list is not every contact category exactly once"),
    ),
)]
pub(crate) async fn order_categories(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<Order>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let old =
        sqlx::query_scalar!("SELECT id FROM contact_categories ORDER BY sort_order, id FOR UPDATE")
            .fetch_all(&mut *tx)
            .await?;
    if !body.matches(&old) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    sqlx::query!(
        "UPDATE contact_categories SET sort_order = ordered.sort_order
         FROM unnest($1::uuid[]) WITH ORDINALITY AS ordered (id, sort_order)
         WHERE contact_categories.id = ordered.id",
        &body.ids,
    )
    .execute(&mut *tx)
    .await?;
    if old != body.ids {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "contact_category.order",
            ("contact_category", None),
            json!([{ "field": "order", "from": old, "to": body.ids }]),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// What a User writes in an Important Contact.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportantContactFields {
    /// 1–100 characters.
    name: String,
    /// The contact category it is listed under.
    category: Uuid,
    /// Up to 30 characters with at least three digits, such as 112, and only digits, spaces
    /// and `+-()`.
    phone: String,
    /// Up to 150 characters, such as opening hours.
    #[serde(default)]
    note: Option<String>,
    /// The place within the category, smallest first; left out, a new contact or one moved to
    /// another category goes last there, and any other edit keeps the place.
    #[serde(default)]
    order: Option<i32>,
}

impl ImportantContactFields {
    /// Trims every text, blank to null, and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let invalid = || ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY);
        let name = self.name.trim();
        let phone = self.phone.trim();
        let note = self
            .note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty());
        if name.is_empty()
            || name.chars().count() > 100
            || phone.len() > 30
            || phone.bytes().filter(u8::is_ascii_digit).count() < 3
            || !phone
                .bytes()
                .all(|b| b.is_ascii_digit() || b" +-()".contains(&b))
            || note.is_some_and(|note| note.chars().count() > 150)
        {
            return Err(invalid());
        }
        Ok(Self {
            name: name.to_owned(),
            category: self.category,
            phone: phone.to_owned(),
            note: note.map(str::to_owned),
            order: self.order,
        })
    }

    fn audited(&self, order: i32) -> Value {
        json!({
            "name": self.name, "category": self.category, "phone": self.phone,
            "note": self.note, "order": order,
        })
    }
}

/// An Important Contact as the admin area lists and edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportantContact {
    id: Uuid,
    #[serde(flatten)]
    fields: ImportantContactFields,
    published: bool,
}

/// Every Important Contact, Drafts and Published, category by category in order.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/important-contacts",
    operation_id = "listImportantContacts",
    tag = "content",
    responses(
        (status = OK, description = "Every Important Contact", body = Vec<ImportantContact>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<ImportantContact>>, ApiError> {
    ensure_writer(&session)?;
    let rows = sqlx::query!(
        "SELECT contact.id, contact.name, category_id, phone, note, contact.sort_order, published
         FROM important_contacts contact
         JOIN contact_categories category ON category.id = category_id
         ORDER BY category.sort_order, category.id, contact.sort_order, contact.name, contact.id"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| ImportantContact {
                id: row.id,
                fields: ImportantContactFields {
                    name: row.name,
                    category: row.category_id,
                    phone: row.phone,
                    note: row.note,
                    order: Some(row.sort_order),
                },
                published: row.published,
            })
            .collect(),
    ))
}

/// Saves a new Important Contact as a Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `422`
/// for a blank name, a malformed phone or note, or an unknown category.
#[utoipa::path(
    post,
    path = "/important-contacts",
    operation_id = "createImportantContact",
    tag = "content",
    request_body = ImportantContactFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<ImportantContactFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let created = saved(
        sqlx::query!(
            "INSERT INTO important_contacts (name, category_id, phone, note, sort_order)
             VALUES ($1, $2, $3, $4, coalesce($5, (SELECT coalesce(max(sort_order), 0) + 1
                 FROM important_contacts WHERE category_id = $2)))
             RETURNING id, sort_order",
            new.name,
            new.category,
            new.phone,
            new.note,
            new.order,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "important_contact.create",
        ("important_contact", Some(&created.id.to_string())),
        diff(&Value::Null, &new.audited(created.sort_order)).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id: created.id })))
}

/// Saves an Important Contact, a Draft or a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published contact, `404` for an unknown contact,
/// `422` for a blank name, a malformed phone or note, or an unknown category.
#[utoipa::path(
    put,
    path = "/important-contacts/{id}",
    operation_id = "updateImportantContact",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Important Contact")),
    request_body = ImportantContactFields,
    responses(
        (status = NO_CONTENT, description = "The Important Contact is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this Important Contact"),
        (status = NOT_FOUND, description = "There is no such Important Contact"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<ImportantContactFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT name, category_id, phone, note, sort_order, published
         FROM important_contacts WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        ensure_publisher(&session)?;
    }
    let order = match new.order {
        Some(order) => order,
        None if new.category == old.category_id => old.sort_order,
        None => {
            sqlx::query_scalar!(
                r#"SELECT coalesce(max(sort_order), 0) + 1 AS "order!" FROM important_contacts
                   WHERE category_id = $1"#,
                new.category,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    saved(
        sqlx::query!(
            "UPDATE important_contacts SET name = $1, category_id = $2, phone = $3, note = $4,
             sort_order = $5 WHERE id = $6",
            new.name,
            new.category,
            new.phone,
            new.note,
            order,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    let from = ImportantContactFields {
        name: old.name,
        category: old.category_id,
        phone: old.phone,
        note: old.note,
        order: None,
    }
    .audited(old.sort_order);
    let changes = diff(&from, &new.audited(order));
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "important_contact.update",
            ("important_contact", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft on the public site.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// contact, `409` when it is already Published.
#[utoipa::path(
    post,
    path = "/important-contacts/{id}/publish",
    operation_id = "publishImportantContact",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Important Contact")),
    responses(
        (status = NO_CONTENT, description = "The Important Contact is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Important Contact"),
        (status = CONFLICT, description = "The Important Contact is already Published"),
    ),
)]
pub(crate) async fn publish(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    set_published(&pool, &session, id, true).await
}

/// Takes a Published contact off the public site, back to Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// contact, `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/important-contacts/{id}/unpublish",
    operation_id = "unpublishImportantContact",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Important Contact")),
    responses(
        (status = NO_CONTENT, description = "The Important Contact is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Important Contact"),
        (status = CONFLICT, description = "The Important Contact is a Draft"),
    ),
)]
pub(crate) async fn unpublish(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    set_published(&pool, &session, id, false).await
}

async fn set_published(
    pool: &PgPool,
    session: &Session,
    id: Uuid,
    published: bool,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(session)?;
    let mut tx = pool.begin().await?;
    let was = sqlx::query_scalar!(
        "SELECT published FROM important_contacts WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!(
        "UPDATE important_contacts SET published = $1 WHERE id = $2",
        published,
        id,
    )
    .execute(&mut *tx)
    .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        if published {
            "important_contact.publish"
        } else {
            "important_contact.unpublish"
        },
        ("important_contact", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good. A Published contact is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// contact, `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/important-contacts/{id}",
    operation_id = "deleteImportantContact",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Important Contact")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Important Contact"),
        (status = CONFLICT, description = "The Important Contact is Published"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT name, published FROM important_contacts WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!("DELETE FROM important_contacts WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "important_contact.delete",
        ("important_contact", Some(&id.to_string())),
        json!([{ "field": "name", "from": old.name, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A Published Important Contact as the public reads it.
#[derive(Serialize, ToSchema)]
pub struct PublishedContact {
    name: String,
    phone: String,
    note: Option<String>,
}

/// One contact category's Published contacts, in order.
#[derive(Serialize, ToSchema)]
pub struct ContactGroup {
    category: String,
    contacts: Vec<PublishedContact>,
}

/// Published Important Contacts grouped by category, each in order; a category without a
/// Published contact is left out.
#[utoipa::path(
    get,
    path = "/important-contacts/published",
    operation_id = "listPublishedImportantContacts",
    tag = "content",
    responses((status = OK, description = "Every Published Important Contact", body = Vec<ContactGroup>)),
)]
pub(crate) async fn list_published(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<ContactGroup>>, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT category.name AS category, contact.name, phone, note
           FROM important_contacts contact
           JOIN contact_categories category ON category.id = category_id
           WHERE published
           ORDER BY category.sort_order, category.id, contact.sort_order, contact.name,
               contact.id"#
    )
    .fetch_all(&pool)
    .await?;
    let mut groups: Vec<ContactGroup> = Vec::new();
    for row in rows {
        let contact = PublishedContact {
            name: row.name,
            phone: row.phone,
            note: row.note,
        };
        // Category names are unique, so a new name starts the next group.
        match groups.last_mut() {
            Some(group) if group.category == row.category => group.contacts.push(contact),
            _ => groups.push(ContactGroup {
                category: row.category,
                contacts: vec![contact],
            }),
        }
    }
    Ok(Json(groups))
}
