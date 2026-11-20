mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a User holding `village.manage`; their session cookie.
async fn manager(pool: &PgPool) -> (TestApp, String) {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(pool, "operator", &["village.manage"]).await;
    (app, cookie)
}

async fn positions(app: &TestApp, cookie: &str) -> Vec<Value> {
    let reply = app.call("GET", "/positions", None, Some(cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.body.as_array().unwrap().clone()
}

/// The id of the Position named `name`.
async fn position(app: &TestApp, cookie: &str, name: &str) -> String {
    positions(app, cookie)
        .await
        .into_iter()
        .find(|position| position["name"] == name)
        .unwrap_or_else(|| panic!("no Position {name}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A Tenure body for `name` on `position` from `start` to `end`.
fn tenure(position: &str, name: &str, kind: &str, start: &str, end: Option<&str>) -> Value {
    json!({
        "positionId": position, "name": name, "kind": kind,
        "startDate": start, "endDate": end,
        "decreeNumber": "141/01/2025", "decreeDate": start,
    })
}

async fn add_tenure(app: &TestApp, cookie: &str, body: &Value) -> Reply {
    app.call("POST", "/tenures", Some(body), Some(cookie)).await
}

async fn hamlet(app: &TestApp, cookie: &str, name: &str) -> String {
    let body = json!({ "kind": "hamlet", "name": name });
    let reply = app
        .call("POST", "/regions/units", Some(&body), Some(cookie))
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);
    reply.body["id"].as_str().unwrap().to_owned()
}

async fn set_hamlet_active(
    app: &TestApp,
    cookie: &str,
    id: &str,
    name: &str,
    active: bool,
) -> StatusCode {
    let body = json!({ "name": name, "active": active });
    app.call(
        "PUT",
        &format!("/regions/units/{id}"),
        Some(&body),
        Some(cookie),
    )
    .await
    .status
}

#[sqlx::test]
async fn the_standard_positions_are_seeded_and_the_head_of_village_stays(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;

    let names: Vec<Value> = positions(&app, &cookie)
        .await
        .iter()
        .map(|position| position["name"].clone())
        .collect();
    assert_eq!(
        names,
        json!([
            "Kepala Desa",
            "Sekretaris Desa",
            "Kaur Tata Usaha dan Umum",
            "Kaur Keuangan",
            "Kaur Perencanaan",
            "Kasi Pemerintahan",
            "Kasi Kesejahteraan",
            "Kasi Pelayanan",
        ])
        .as_array()
        .unwrap()
        .clone()
    );

    let head = position(&app, &cookie, "Kepala Desa").await;
    let reply = app
        .call("DELETE", &format!("/positions/{head}"), None, Some(&cookie))
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);

    // A Village-defined Position is added at the end, renamed, and deleted while unused.
    let reply = app
        .call(
            "POST",
            "/positions",
            Some(&json!({ "name": " Staf Pelayanan " })),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let staff = reply.body["id"].as_str().unwrap().to_owned();
    let rename = json!({ "name": "Staf Kasi Pelayanan" });
    let reply = app
        .call(
            "PUT",
            &format!("/positions/{staff}"),
            Some(&rename),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let last = positions(&app, &cookie).await.pop().unwrap();
    assert_eq!(
        (&last["id"], &last["name"]),
        (&json!(staff), &json!("Staf Kasi Pelayanan"))
    );
    let reply = app
        .call(
            "DELETE",
            &format!("/positions/{staff}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);

    // One that ever had a Tenure stays, since Tenures are never deleted.
    let secretary = position(&app, &cookie, "Sekretaris Desa").await;
    let body = tenure(&secretary, "Siti", "definitive", "2024-01-01", None);
    assert_eq!(
        add_tenure(&app, &cookie, &body).await.status,
        StatusCode::CREATED
    );
    let reply = app
        .call(
            "DELETE",
            &format!("/positions/{secretary}"),
            None,
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn only_a_village_manager_changes_the_government(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let reader = app.user_with(&pool, "reader", &[]).await;
    let head = position(&app, &cookie, "Kepala Desa").await;

    assert_eq!(positions(&app, &reader).await.len(), 8);
    let body = tenure(&head, "Budi", "definitive", "2024-01-01", None);
    assert_eq!(
        add_tenure(&app, &reader, &body).await.status,
        StatusCode::FORBIDDEN
    );
    let reply = app
        .call(
            "POST",
            "/positions",
            Some(&json!({ "name": "X" })),
            Some(&reader),
        )
        .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(
        app.request("GET", "/positions").await,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
async fn a_tenure_keeps_its_record_and_the_official_their_name(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let head = position(&app, &cookie, "Kepala Desa").await;

    let body = json!({
        "positionId": head, "name": " Budi Santoso ", "nipd": " 1101012001001 ",
        "kind": "definitive", "startDate": "2024-01-01", "endDate": null,
        "decreeNumber": " 141/01/2024 ", "decreeDate": "2023-12-20",
    });
    let reply = add_tenure(&app, &cookie, &body).await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let id = reply.body["id"].as_str().unwrap().to_owned();

    let tenures = &positions(&app, &cookie).await[0]["tenures"];
    let official = tenures[0]["officialId"].clone();
    assert_eq!(
        tenures,
        &json!([{
            "id": id, "officialId": official, "name": "Budi Santoso",
            "nipd": "1101012001001", "kind": "definitive", "startDate": "2024-01-01",
            "endDate": null, "decreeNumber": "141/01/2024", "decreeDate": "2023-12-20",
            "photo": null, "photoAlt": null,
        }])
    );

    // An end before the start, a blank name or decree, or a photo without alt text is refused.
    for (field, value) in [
        ("endDate", json!("2023-12-31")),
        ("name", json!(" ")),
        ("decreeNumber", json!("")),
        ("kind", json!("interim")),
    ] {
        let mut bad = body.clone();
        bad[field] = value;
        let status = add_tenure(&app, &cookie, &bad).await.status;
        assert!(status.is_client_error(), "{field}: {status}");
    }
    let photo = app
        .upload("/tenures/photo", common::png(64, 64), &cookie)
        .await;
    assert_eq!(photo.status, StatusCode::CREATED);
    let mut no_alt = body.clone();
    no_alt["photo"] = photo.body["id"].clone();
    no_alt["kind"] = json!("acting");
    assert_eq!(
        add_tenure(&app, &cookie, &no_alt).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Editing ends the Tenure, adds the photo, and corrects the Official's name.
    let edit = json!({
        "name": "Budi Santosa", "nipd": null, "kind": "definitive",
        "startDate": "2024-01-01", "endDate": "2025-06-30",
        "decreeNumber": "141/01/2024", "decreeDate": "2023-12-20",
        "photo": photo.body["id"], "photoAlt": "Budi Santosa berbaju dinas",
    });
    let reply = app
        .call("PUT", &format!("/tenures/{id}"), Some(&edit), Some(&cookie))
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    let tenure = &positions(&app, &cookie).await[0]["tenures"][0];
    assert_eq!(
        (&tenure["name"], &tenure["endDate"], &tenure["photo"]),
        (
            &json!("Budi Santosa"),
            &json!("2025-06-30"),
            &photo.body["id"]
        )
    );

    // The same Official later holds the secretary's Position too.
    let secretary = position(&app, &cookie, "Sekretaris Desa").await;
    let mut again = tenure_body_for(&secretary, "2025-07-01");
    again["officialId"] = official.clone();
    again["name"] = json!("Budi Santosa");
    assert_eq!(
        add_tenure(&app, &cookie, &again).await.status,
        StatusCode::CREATED
    );
    let held = positions(&app, &cookie).await[1]["tenures"][0]["officialId"].clone();
    assert_eq!(held, official);

    assert!(
        sqlx::query("DELETE FROM tenures")
            .execute(&pool)
            .await
            .is_err()
    );
}

fn tenure_body_for(position: &str, start: &str) -> Value {
    tenure(position, "Siapa", "definitive", start, None)
}

#[sqlx::test]
async fn at_most_one_definitive_or_appointee_tenure_serves_at_a_time(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let head = position(&app, &cookie, "Kepala Desa").await;
    let add = |body: Value| {
        let (app, cookie) = (&app, &cookie);
        async move { add_tenure(app, cookie, &body).await.status }
    };

    let first = add_tenure(
        &app,
        &cookie,
        &tenure(
            &head,
            "Budi",
            "definitive",
            "2020-01-01",
            Some("2025-12-31"),
        ),
    )
    .await;
    assert_eq!(first.status, StatusCode::CREATED);

    // An overlapping Definitif or Pj. is refused, at either end.
    for (kind, start, end) in [
        ("definitive", "2025-12-31", None),
        ("appointee", "2019-01-01", Some("2020-01-01")),
        ("appointee", "2021-01-01", Some("2021-02-01")),
    ] {
        assert_eq!(
            add(tenure(&head, "Ani", kind, start, end)).await,
            StatusCode::CONFLICT,
            "{kind} {start}"
        );
    }
    // A Plt. may overlap, and a Pj. may follow the day after.
    assert_eq!(
        add(tenure(
            &head,
            "Cahya",
            "acting",
            "2025-06-01",
            Some("2025-08-01")
        ))
        .await,
        StatusCode::CREATED
    );
    let appointee = add_tenure(
        &app,
        &cookie,
        &tenure(&head, "Dewi", "appointee", "2026-01-01", None),
    )
    .await;
    assert_eq!(appointee.status, StatusCode::CREATED);

    // Reopening the first Tenure would overlap the Pj., so it is refused; another Position
    // is unaffected.
    let first = first.body["id"].as_str().unwrap();
    let reopen = json!({
        "name": "Budi", "kind": "definitive", "startDate": "2020-01-01", "endDate": null,
        "decreeNumber": "141/01/2025", "decreeDate": "2020-01-01",
    });
    let reply = app
        .call(
            "PUT",
            &format!("/tenures/{first}"),
            Some(&reopen),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    // Editing a Tenure does not collide with itself.
    let mut same = reopen.clone();
    same["endDate"] = json!("2025-12-30");
    let reply = app
        .call(
            "PUT",
            &format!("/tenures/{first}"),
            Some(&same),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);

    let secretary = position(&app, &cookie, "Sekretaris Desa").await;
    assert_eq!(
        add(tenure(&secretary, "Eka", "definitive", "2020-01-01", None)).await,
        StatusCode::CREATED
    );
}

#[sqlx::test]
async fn a_hamlet_brings_its_head_position_and_leaves_only_once_its_head_has_ended(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;

    let all = positions(&app, &cookie).await;
    let head = all.last().unwrap();
    assert_eq!(
        (&head["name"], &head["hamletId"], &head["active"]),
        (&json!("Kepala Dusun Krajan"), &json!(krajan), &json!(true))
    );
    let head = head["id"].as_str().unwrap().to_owned();

    // Bound to its Hamlet: neither renamed nor deleted.
    let reply = app
        .call(
            "PUT",
            &format!("/positions/{head}"),
            Some(&json!({ "name": "X" })),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    let reply = app
        .call("DELETE", &format!("/positions/{head}"), None, Some(&cookie))
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);

    let serving = add_tenure(
        &app,
        &cookie,
        &tenure(&head, "Pak Sastro", "definitive", "2024-01-01", None),
    )
    .await;
    assert_eq!(serving.status, StatusCode::CREATED);
    assert_eq!(
        set_hamlet_active(&app, &cookie, &krajan, "Krajan", false).await,
        StatusCode::CONFLICT
    );

    let id = serving.body["id"].as_str().unwrap();
    let ended = json!({
        "name": "Pak Sastro", "kind": "definitive", "startDate": "2024-01-01",
        "endDate": "2025-01-31", "decreeNumber": "141/01/2025", "decreeDate": "2024-01-01",
    });
    let reply = app
        .call(
            "PUT",
            &format!("/tenures/{id}"),
            Some(&ended),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(
        set_hamlet_active(&app, &cookie, &krajan, "Krajan", false).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        positions(&app, &cookie).await.last().unwrap()["active"],
        json!(false)
    );

    // Its inactive Position takes no new Tenure, nor reopens an ended one.
    assert_eq!(
        add_tenure(
            &app,
            &cookie,
            &tenure(&head, "Bu Ratna", "acting", "2025-02-01", None)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    let mut reopened = ended.clone();
    reopened["endDate"] = Value::Null;
    let reply = app
        .call(
            "PUT",
            &format!("/tenures/{id}"),
            Some(&reopened),
            Some(&cookie),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);

    // Reactivated with its Hamlet.
    assert_eq!(
        set_hamlet_active(&app, &cookie, &krajan, "Krajan", true).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        positions(&app, &cookie).await.last().unwrap()["active"],
        json!(true)
    );
}

#[sqlx::test]
async fn the_public_sees_serving_officials_and_hamlet_heads_without_nipd_or_decree(pool: PgPool) {
    let (app, cookie) = manager(&pool).await;
    let krajan = hamlet(&app, &cookie, "Krajan").await;
    let kidul = hamlet(&app, &cookie, "Kidul").await;
    let closed = hamlet(&app, &cookie, "Lama").await;
    assert_eq!(
        set_hamlet_active(&app, &cookie, &closed, "Lama", false).await,
        StatusCode::NO_CONTENT
    );
    let head = position(&app, &cookie, "Kepala Desa").await;
    let secretary = position(&app, &cookie, "Sekretaris Desa").await;
    let krajan_head = position(&app, &cookie, "Kepala Dusun Krajan").await;

    let photo = app
        .upload("/tenures/photo", common::png(64, 64), &cookie)
        .await;
    let mut serving = tenure(&head, "Budi", "appointee", "2020-01-01", None);
    serving["nipd"] = json!("1101012001001");
    serving["photo"] = photo.body["id"].clone();
    serving["photoAlt"] = json!("Budi");
    for body in [
        serving,
        tenure(&head, "Ani", "definitive", "2010-01-01", Some("2015-12-31")),
        tenure(&secretary, "Siti", "definitive", "2999-01-01", None),
        tenure(&secretary, "Cahya", "acting", "2020-01-01", None),
        tenure(&krajan_head, "Pak Sastro", "definitive", "2020-01-01", None),
    ] {
        assert_eq!(
            add_tenure(&app, &cookie, &body).await.status,
            StatusCode::CREATED
        );
    }

    let (status, body) = app.get("/government").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "officials": [
                { "name": "Budi", "position": "Kepala Desa", "kind": "appointee",
                  "photo": photo.body["id"], "photoAlt": "Budi" },
                { "name": "Cahya", "position": "Sekretaris Desa", "kind": "acting",
                  "photo": null, "photoAlt": null },
                { "name": "Pak Sastro", "position": "Kepala Dusun Krajan", "kind": "definitive",
                  "photo": null, "photoAlt": null },
            ],
            "hamletLabel": "dusun",
            "hamlets": [
                { "id": krajan, "name": "Krajan", "heads": ["Pak Sastro"] },
                { "id": kidul, "name": "Kidul", "heads": [] },
            ],
        })
    );
}
