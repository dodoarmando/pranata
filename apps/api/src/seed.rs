//! The synthetic content the demo Installation and the `perf` job share: a 2400 px hero with
//! Quick Links, 10 News Posts (one of them long), 3 Announcements in effect, 5 upcoming
//! Events and one Complaint. No real village or person appears in it.
//!
//! It goes through the HTTP API in process, signed in as a User of its own, so every rule a
//! publisher meets applies to it too.

use std::{io::Cursor, path::Path};

use anyhow::{Context, bail, ensure};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::{Days, NaiveDateTime, NaiveTime, Utc};
use image::{ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::public_form::{find_number, hex};

/// The User the content is written as; its password is random and thrown away.
const USERNAME: &str = "redaksi";

/// Fills an Installation whose Village is set and which has no News Post yet, and returns the
/// seeded Complaint's code.
///
/// # Errors
/// When the Installation already holds News Posts, or the API refuses any step.
pub async fn run(pool: &PgPool, public_origin: &str, files_dir: &Path) -> anyhow::Result<String> {
    let has_news = sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM news_posts) AS "e!""#)
        .fetch_one(pool)
        .await?;
    ensure!(
        !has_news,
        "the Installation already has News Posts; seed only an empty one"
    );

    let api = Api {
        router: crate::app(pool.clone(), public_origin.to_owned(), files_dir.to_owned())?,
        cookie: None,
    };
    let token = crate::user::create_administrator(pool, USERNAME).await?;
    let mut password = [0u8; 24];
    getrandom::fill(&mut password)?;
    let password = hex(&password);
    api.send(
        "POST",
        "/auth/access-link",
        &json!({ "token": token, "password": password }),
    )
    .await?;
    let api = api.signed_in(&password).await?;

    let hero = api.upload("/home/hero/photo", photo(2400, 1350, 0)).await?;
    api.send("PUT", "/home/hero", &json!({
        "photo": hero,
        "photoAlt": "Hamparan sawah hijau di kaki bukit saat pagi",
        "title": "Desa yang terbuka, melayani, dan mandiri",
        "subtitle": "Ikuti berita, agenda kegiatan, dokumen resmi, dan penggunaan anggaran desa.",
        "buttonLabel": "Profil desa",
        "buttonPath": "/profil",
        "quickLinks": [
            { "label": "Berita", "path": "/berita" },
            { "label": "Pengumuman", "path": "/pengumuman" },
            { "label": "Agenda", "path": "/agenda" },
            { "label": "Pengaduan", "path": "/pengaduan" },
        ],
    }))
    .await?;

    let mut categories = Vec::new();
    for name in ["Pemerintahan", "Kegiatan Warga", "Pembangunan"] {
        categories.push(
            api.send("POST", "/categories", &json!({ "name": name }))
                .await?["id"]
                .clone(),
        );
    }

    for (index, title) in NEWS.iter().enumerate() {
        let cover = api
            .upload("/news-posts/cover", photo(1600, 900, index + 1))
            .await?;
        let body = if index == 0 {
            long_body()
        } else {
            short_body(title)
        };
        api.publish("news-posts", &json!({
            "title": title,
            "category": categories[index % categories.len()],
            "summary": format!("{title}. Ringkasan kabar ini ditulis sebagai contoh isi situs desa."),
            "cover": cover,
            "coverAlt": format!("Foto kegiatan: {title}"),
            "coverCaption": "Foto contoh untuk situs demo.",
            "body": body,
        }))
        .await?;
    }

    let today = Utc::now().date_naive();
    for (title, summary) in ANNOUNCEMENTS {
        api.publish(
            "announcements",
            &json!({
                "title": title,
                "summary": summary,
                "startDate": today - Days::new(1),
                "endDate": today + Days::new(90),
            }),
        )
        .await?;
    }

    for (index, (title, location)) in (0u64..).zip(EVENTS) {
        let day = today + Days::new(3 + index * 7);
        let starts_at = NaiveDateTime::new(day, NaiveTime::from_hms_opt(8, 0, 0).context("time")?);
        api.publish(
            "events",
            &json!({
                "title": title,
                "category": categories[1],
                "startsAt": starts_at,
                "endsAt": starts_at + chrono::TimeDelta::hours(3),
                "location": location,
                "description": "Kegiatan contoh untuk situs demo. Warga dipersilakan hadir.",
            }),
        )
        .await?;
    }

    complaint(&api).await
}

/// Sends a Complaint as a resident does, proof of work included, and returns its code.
async fn complaint(api: &Api) -> anyhow::Result<String> {
    let form = api.get("/complaints/form").await?;
    let topic = form["topics"][0]["id"].clone();
    let challenge = api.get("/public-form/challenge").await?;
    let salt = text(&challenge["salt"])?;
    let max = challenge["maxNumber"].as_u64().context("maxNumber")?;
    let number = find_number(salt, text(&challenge["challenge"])?, u32::try_from(max)?)
        .context("the challenge has no solution")?;
    let reply = api
        .send(
            "POST",
            "/complaints",
            &json!({
                "topicId": topic,
                "text": "Lampu jalan di dekat balai warga mati sejak pekan lalu. Mohon diperbaiki.",
                "proof": { "salt": salt, "number": number, "signature": challenge["signature"] },
            }),
        )
        .await?;
    Ok(text(&reply["code"])?.to_owned())
}

/// The API in process, with the session cookie once signed in.
struct Api {
    router: Router,
    cookie: Option<String>,
}

impl Api {
    async fn signed_in(self, password: &str) -> anyhow::Result<Self> {
        let request = self.request("POST", "/auth/sign-in").body(Body::from(
            json!({ "username": USERNAME, "password": password, "remember": false }).to_string(),
        ))?;
        let response = self.router.clone().oneshot(request).await?;
        ensure!(
            response.status() == StatusCode::NO_CONTENT,
            "sign-in: {}",
            response.status()
        );
        let cookie = response.headers()[header::SET_COOKIE].to_str()?;
        let cookie = cookie.split(';').next().unwrap_or_default().to_owned();
        Ok(Self {
            cookie: Some(cookie),
            ..self
        })
    }

    /// A same-origin request, as the web app sends it.
    fn request(&self, method: &str, path: &str) -> axum::http::request::Builder {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("sec-fetch-site", "same-origin")
            .header(header::CONTENT_TYPE, "application/json");
        match &self.cookie {
            Some(cookie) => request.header(header::COOKIE, cookie),
            None => request,
        }
    }

    /// Creates an item of `collection`, such as `events`, and publishes it.
    async fn publish(&self, collection: &str, body: &Value) -> anyhow::Result<()> {
        let id = self.send("POST", &format!("/{collection}"), body).await?;
        let path = format!("/{collection}/{}/publish", text(&id["id"])?);
        self.send("POST", &path, &json!({})).await?;
        Ok(())
    }

    async fn get(&self, path: &str) -> anyhow::Result<Value> {
        self.exchange("GET", path, self.request("GET", path).body(Body::empty())?)
            .await
    }

    async fn send(&self, method: &str, path: &str, body: &Value) -> anyhow::Result<Value> {
        let request = self
            .request(method, path)
            .body(Body::from(body.to_string()))?;
        self.exchange(method, path, request).await
    }

    /// Uploads an image and returns its file id.
    async fn upload(&self, path: &str, bytes: Vec<u8>) -> anyhow::Result<Value> {
        let request = self
            .request("POST", path)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(bytes))?;
        Ok(self.exchange("POST", path, request).await?["id"].clone())
    }

    async fn exchange(
        &self,
        method: &str,
        path: &str,
        request: Request<Body>,
    ) -> anyhow::Result<Value> {
        let response = self.router.clone().oneshot(request).await?;
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        if !status.is_success() {
            bail!(
                "{method} {path}: {status} {}",
                String::from_utf8_lossy(&bytes)
            );
        }
        Ok(serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }
}

fn text(value: &Value) -> anyhow::Result<&str> {
    value
        .as_str()
        .with_context(|| format!("expected a string, got {value}"))
}

/// A JPEG of a hilly landscape, varied by `seed`, with grain so it weighs what a photo does.
fn photo(width: u32, height: u32, seed: usize) -> Vec<u8> {
    let shift = f64::from(u32::try_from(seed % 7).unwrap_or(0));
    let (w, h) = (f64::from(width), f64::from(height));
    let bands = [
        (0.55, [127, 174, 122]),
        (0.68, [78, 138, 92]),
        (0.82, [45, 99, 70]),
    ];
    let salt = 0x9e37_79b9_u32 ^ u32::try_from(seed).unwrap_or(0);
    // Grain in blocks a 400th of the width, coarse enough to survive the smaller variants.
    let block = (width / 400).max(1);
    let image = RgbImage::from_fn(width, height, |x, y| {
        let (fx, fy) = (f64::from(x) / w, f64::from(y) / h);
        let mut colour = [150.0 + 80.0 * fy, 195.0 + 30.0 * fy, 235.0 - 10.0 * fy];
        for (index, (level, band)) in (0u8..).zip(bands) {
            let wave = 0.04 * (fx * (5.0 + f64::from(index)) + shift).sin()
                + 0.015 * (fx * 23.0 + shift * 2.0).sin();
            if fy > level + wave {
                colour = band.map(f64::from);
            }
        }
        // xorshift of the block's position: grain of ±12 per channel.
        let mut state =
            salt ^ (x / block).wrapping_mul(73_856_093) ^ (y / block).wrapping_mul(19_349_663);
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let grain = f64::from(state % 25) - 12.0;
        Rgb(colour.map(|c| {
            // Clamped to 0..=255 first, so the cast never truncates.
            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let channel = (c + grain).clamp(0.0, 255.0) as u8;
            channel
        }))
    });
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
        .expect("encoding a JPEG in memory cannot fail");
    bytes
}

fn paragraph(text: &str) -> Value {
    json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
}

fn short_body(title: &str) -> Value {
    json!({ "type": "doc", "content": [
        paragraph(&format!("{title}. Kegiatan ini diikuti perangkat desa bersama warga dari setiap dusun.")),
        paragraph("Pemerintah desa mengucapkan terima kasih kepada semua pihak yang telah membantu."),
    ]})
}

/// About 1,500 words over headings, lists, a quote and a table: the long News Post `perf`
/// measures.
fn long_body() -> Value {
    let minutes = "Musyawarah desa membahas rencana kerja tahun depan secara terbuka. Setiap dusun \
        menyampaikan usulan yang telah dibahas di tingkat RT dan RW, lalu usulan itu disusun \
        menurut prioritas: kebutuhan dasar, pelayanan umum, dan pengembangan ekonomi warga. \
        Badan Permusyawaratan Desa mencatat setiap keputusan agar dapat diawasi bersama.";
    let mut content = vec![paragraph(minutes)];
    for section in [
        "Latar belakang",
        "Usulan dari dusun",
        "Prioritas anggaran",
        "Tindak lanjut",
    ] {
        content.push(json!({ "type": "heading", "attrs": { "level": 2 },
            "content": [{ "type": "text", "text": section }] }));
        for _ in 0..4 {
            content.push(paragraph(minutes));
        }
        content.push(json!({ "type": "bulletList", "content": [
            { "type": "listItem", "content": [paragraph("Perbaikan saluran air di dua dusun")] },
            { "type": "listItem", "content": [paragraph("Pelatihan usaha bagi kelompok perempuan")] },
            { "type": "listItem", "content": [paragraph("Penambahan lampu jalan di jalur sekolah")] },
        ]}));
    }
    content.push(json!({ "type": "blockquote", "content": [
        paragraph("Pembangunan desa berjalan baik bila warga ikut merencanakan dan mengawasinya."),
    ]}));
    let cell = |kind: &str, text: &str| json!({ "type": kind, "content": [paragraph(text)] });
    content.push(json!({ "type": "table", "content": [
        { "type": "tableRow", "content": [cell("tableHeader", "Bidang"), cell("tableHeader", "Usulan")] },
        { "type": "tableRow", "content": [cell("tableCell", "Pembangunan"), cell("tableCell", "12")] },
        { "type": "tableRow", "content": [cell("tableCell", "Pemberdayaan"), cell("tableCell", "8")] },
        { "type": "tableRow", "content": [cell("tableCell", "Pelayanan"), cell("tableCell", "5")] },
    ]}));
    content.push(paragraph(minutes));
    json!({ "type": "doc", "content": content })
}

const NEWS: [&str; 10] = [
    "Musyawarah desa menyepakati rencana kerja tahun depan",
    "Gotong royong membersihkan saluran irigasi menjelang musim tanam",
    "Posyandu balita kini buka dua kali sebulan",
    "Jalan usaha tani sepanjang 800 meter selesai dibangun",
    "Pelatihan pengolahan hasil panen bagi kelompok perempuan",
    "Penyaluran bantuan langsung tunai tahap ketiga",
    "Lomba kebersihan antar-RT meriahkan hari jadi desa",
    "Perpustakaan desa menambah koleksi buku anak",
    "Pemeriksaan kesehatan gratis bagi warga lanjut usia",
    "Kelompok tani menerima bantuan bibit unggul",
];

const ANNOUNCEMENTS: [(&str, &str); 3] = [
    (
        "Jadwal pelayanan kantor desa",
        "Kantor desa melayani warga Senin–Jumat pukul 08.00–15.00.",
    ),
    (
        "Pendaftaran pelatihan menjahit",
        "Warga usia 18–45 tahun dapat mendaftar di kantor desa.",
    ),
    (
        "Pemadaman listrik terjadwal",
        "Listrik di dusun bagian utara padam Sabtu pukul 09.00–12.00.",
    ),
];

const EVENTS: [(&str, &str); 5] = [
    ("Posyandu balita", "Balai dusun"),
    ("Kerja bakti bulanan", "Lapangan desa"),
    ("Musyawarah dusun", "Balai desa"),
    ("Senam lansia", "Halaman kantor desa"),
    ("Penyuluhan pertanian", "Balai kelompok tani"),
];
