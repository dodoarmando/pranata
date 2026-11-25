import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
	type APIRequestContext,
	type Browser,
	type BrowserContext,
	expect,
	type Page,
	request,
	test,
} from "@playwright/test";
import exifr from "exifr";
import { analyzeAxe } from "./axe";
import {
	ADMIN_PASSWORD,
	ADMIN_USERNAME,
	accessLink,
	ORIGIN,
	OVERVIEW_TEXT,
	VILLAGE_HEADING,
} from "./stack";

const PHOTO = resolve(import.meta.dirname, "fixtures/gps-photo.jpg");
const EXPIRED =
	"Tautan ini sudah tidak berlaku. Minta tautan baru kepada Administrator.";
const HEIC_ERROR = "Format foto tidak didukung. Gunakan JPEG, PNG, atau WebP.";
// An `ftyp` box with a HEIC brand: what an iPhone photo starts with.
const FAKE_HEIC = Buffer.concat([
	Buffer.from([0, 0, 0, 24]),
	Buffer.from("ftypheic"),
	Buffer.alloc(4),
	Buffer.from("mif1heic"),
]);

interface Variant {
	url: string;
	width: number;
}
interface Hero {
	id: string;
	variants: Variant[];
}
interface Profile {
	hero?: Hero | null;
	overview?: unknown;
	updated_at: string;
}

test.use({ viewport: { width: 390, height: 844 } });
test.describe.configure({ mode: "serial" });

async function signIn(page: Page) {
	await page.goto("/masuk");
	await page.getByLabel("Nama pengguna").fill(ADMIN_USERNAME);
	await page.getByLabel("Kata sandi", { exact: true }).fill(ADMIN_PASSWORD);
	await page.getByRole("button", { name: "Masuk", exact: true }).click();
	await expect(page).toHaveURL(/\/admin$/);
}

// A second, independent session over the API only (its own cookie jar).
async function apiSession(): Promise<{
	api: APIRequestContext;
	csrf: string;
}> {
	const api = await request.newContext({
		baseURL: ORIGIN,
		ignoreHTTPSErrors: true,
	});
	const login = await api.post("/api/auth/login", {
		data: { username: ADMIN_USERNAME, password: ADMIN_PASSWORD },
	});
	expect(login.ok()).toBeTruthy();
	const { csrf_token: csrf } = (await login.json()) as { csrf_token: string };
	return { api, csrf };
}

async function overviewEditor(page: Page) {
	const editor = page.getByRole("textbox", { name: "Gambaran umum" });
	await editor.click();
	return editor;
}

// D-12: the very next anonymous request after a successful save is fresh. No
// polling: the API purges before it answers the PUT. The web cache TTL is 300 s
// and `/` was cached by earlier requests (stack.sh's readiness check, the
// previous test), so without the purge this would read the stale page.
async function expectHomeToContain(visitor: APIRequestContext, text: string) {
	const html = await (await visitor.get(ORIGIN)).text();
	expect(html, `the next request after saving should show "${text}"`).toContain(
		text,
	);
}

function largestVariant(variants: Variant[]): Variant {
	const sorted = [...variants].sort((a, b) => a.width - b.width);
	return sorted[sorted.length - 1] as Variant;
}

// Lists the chunk ids of a WebP (RIFF) file, e.g. VP8X, EXIF, XMP, VP8.
function webpChunks(bytes: Buffer): string[] {
	const ids: string[] = [];
	let offset = 12;
	while (offset + 8 <= bytes.length) {
		ids.push(bytes.toString("ascii", offset, offset + 4).trim());
		offset +=
			8 + bytes.readUInt32LE(offset + 4) + (bytes.readUInt32LE(offset + 4) % 2);
	}
	return ids;
}

// The variants are WebP, which exifr cannot read, so exifr covers whatever it
// understands (and JPEG/PNG should the format change) and the RIFF chunk list
// covers WebP: there must be no EXIF or XMP chunk at all.
async function expectNoExif(bytes: Buffer) {
	if (bytes.toString("ascii", 0, 4) === "RIFF") {
		expect(bytes.toString("ascii", 8, 12)).toBe("WEBP");
		const chunks = webpChunks(bytes);
		expect(chunks).not.toContain("EXIF");
		expect(chunks).not.toContain("XMP");
	}
	const tags = await exifr.parse(bytes, true).catch(() => undefined);
	expect(await exifr.gps(bytes).catch(() => undefined)).toBeUndefined();
	expect(tags?.GPSLatitude).toBeUndefined();
	expect(tags?.Make).toBeUndefined();
}

test.describe("admin profile", () => {
	let context: BrowserContext;
	let page: Page;

	test.beforeAll(async ({ browser }: { browser: Browser }) => {
		context = await browser.newContext({
			viewport: { width: 390, height: 844 },
		});
		page = await context.newPage();
	});

	test.afterAll(async () => {
		await context.close();
	});

	test("access link → set password → sign in", async () => {
		await page.goto(accessLink());
		await expect(
			page.getByRole("heading", { name: "Atur kata sandi" }),
		).toBeVisible();
		// The token must leave the address bar as soon as it is read.
		await expect(page).toHaveURL(/\/akses$/);

		await page.getByLabel("Kata sandi baru").fill(ADMIN_PASSWORD);
		await page.getByLabel("Ulangi kata sandi").fill(ADMIN_PASSWORD);
		await page.getByRole("button", { name: "Simpan kata sandi" }).click();
		await expect(page).toHaveURL(/\/masuk$/);

		await page.getByLabel("Nama pengguna").fill(ADMIN_USERNAME);
		await page.getByLabel("Kata sandi", { exact: true }).fill(ADMIN_PASSWORD);
		await page.getByRole("button", { name: "Masuk", exact: true }).click();
		await expect(page).toHaveURL(/\/admin$/);
	});

	test("second use of access link shows expiry message", async ({
		browser,
	}) => {
		const stranger = await browser.newContext({ ignoreHTTPSErrors: true });
		const other = await stranger.newPage();
		await other.goto(accessLink());
		await other.getByLabel("Kata sandi baru").fill("another-password-456");
		await other.getByLabel("Ulangi kata sandi").fill("another-password-456");
		await other.getByRole("button", { name: "Simpan kata sandi" }).click();
		await expect(other.getByRole("alert")).toHaveText(EXPIRED);
		await stranger.close();
	});

	test("link and video dialogs do not save", async () => {
		const puts: string[] = [];
		page.on("request", (req) => {
			if (req.method() === "PUT") puts.push(req.url());
		});
		await page.goto("/admin/desa/profil");
		const editor = await overviewEditor(page);
		await page.keyboard.type("Kunjungi situs resmi desa");
		await page.keyboard.press("ControlOrMeta+A");

		await page.getByRole("button", { name: "Tautan" }).click();
		await page.getByLabel("Alamat tautan").fill("https://example.org/desa");
		await page.getByRole("button", { name: "Terapkan" }).click();
		await expect(editor.locator("a")).toHaveAttribute(
			"href",
			"https://example.org/desa",
		);

		await page.getByRole("button", { name: "Sisipkan video YouTube" }).click();
		await page
			.getByLabel("Tautan video")
			.fill("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
		await page.getByRole("button", { name: "Sisipkan", exact: true }).click();
		await expect(page.getByRole("dialog")).toHaveCount(0);

		// Give a stray autosave time to fire before asserting it did not.
		await page.waitForTimeout(1500);
		expect(puts).toEqual([]);
		page.removeAllListeners("request");
	});

	test("edit overview with tiptap and save", async ({ browser }) => {
		await page.goto("/admin/desa/profil");
		await overviewEditor(page);
		await page.keyboard.type(OVERVIEW_TEXT);
		const saved = page.waitForResponse(
			(res) => res.request().method() === "PUT" && res.status() === 200,
		);
		await page.getByRole("button", { name: "Simpan", exact: true }).click();
		await saved;
		await expect(page.getByText("Profil desa disimpan.")).toBeVisible();

		// A visitor sees it at once: the save purged the cached page.
		const visitor = await browser.newContext({ ignoreHTTPSErrors: true });
		await expectHomeToContain(visitor.request, OVERVIEW_TEXT);
		const home = await visitor.newPage();
		await home.goto(ORIGIN);
		await expect(home.getByRole("heading", { level: 1 })).toHaveText(
			VILLAGE_HEADING,
		);
		await expect(home.getByText(OVERVIEW_TEXT)).toBeVisible();
		await visitor.close();
	});

	test("a stale save offers to reload", async () => {
		await page.goto("/admin/desa/profil");
		await overviewEditor(page);
		await page.keyboard.type(" Tambahan lokal.");

		// Someone else saves in the meantime (second session over the API).
		const { api, csrf } = await apiSession();
		const current = (await (
			await api.get("/api/admin/village/profile")
		).json()) as Profile;
		const bump = await api.put("/api/admin/village/profile", {
			headers: { "x-csrf-token": csrf },
			data: {
				overview: current.overview ?? null,
				hero_asset_id: current.hero?.id ?? null,
				updated_at: current.updated_at,
			},
		});
		expect(bump.ok()).toBeTruthy();
		await api.dispose();

		await page.getByRole("button", { name: "Simpan", exact: true }).click();
		const alert = page.getByRole("alert").filter({ hasText: "sudah diubah" });
		await expect(alert).toBeVisible();
		await expect(
			page.getByRole("button", {
				name: "Muat ulang data terbaru dan pertahankan perubahan saya",
			}),
		).toBeVisible();
		await expect(
			page.getByRole("button", {
				name: "Muat ulang data terbaru dan buang perubahan saya",
			}),
		).toBeVisible();
		await expect(
			page.getByRole("button", { name: "Simpan", exact: true }),
		).toBeDisabled();

		// Keeping the edits adopts the new token, so saving works again.
		await page
			.getByRole("button", {
				name: "Muat ulang data terbaru dan pertahankan perubahan saya",
			})
			.click();
		await expect(alert).toHaveCount(0);
		const saved = page.waitForResponse(
			(res) => res.request().method() === "PUT" && res.status() === 200,
		);
		await page.getByRole("button", { name: "Simpan", exact: true }).click();
		await saved;
	});

	test("upload phone photo", async ({ browser }) => {
		await page.goto("/admin/desa/profil");
		await page.locator('input[type="file"]').last().setInputFiles(PHOTO);
		await expect(page.getByText("Selesai")).toBeVisible({ timeout: 45_000 });
		await expect(page.getByAltText("Foto hero saat ini")).toBeVisible();
		const saved = page.waitForResponse(
			(res) => res.request().method() === "PUT" && res.status() === 200,
		);
		await page.getByRole("button", { name: "Simpan", exact: true }).click();
		await saved;

		const visitor = await browser.newContext({ ignoreHTTPSErrors: true });
		await expectHomeToContain(visitor.request, "/media/");
		const home = await visitor.newPage();
		await home.goto(ORIGIN);
		const hero = home.locator("section img[srcset]");
		await expect(hero).toBeVisible();
		const url = await hero.getAttribute("src");
		expect(url).toMatch(/^\/media\//);
		const media = await visitor.request.get(
			new URL(url as string, ORIGIN).href,
		);
		expect(media.status()).toBe(200);
		expect(media.headers()["content-type"]).toMatch(/^image\//);
		expect(media.headers()["cache-control"]).toContain("immutable");
		await expectNoExif(await media.body());
		await visitor.close();
	});

	test("original with GPS is stripped by the server", async () => {
		// The browser already re-encodes big photos, which drops EXIF. Send the
		// untouched original straight to the API to prove the server does it too.
		const original = readFileSync(PHOTO);
		expect(await exifr.gps(original)).toBeDefined();

		const { api, csrf } = await apiSession();
		const created = await api.post("/api/admin/media", {
			headers: { "x-csrf-token": csrf },
			multipart: {
				kind: "image",
				file: {
					name: "gps-photo.jpg",
					mimeType: "image/jpeg",
					buffer: original,
				},
			},
		});
		expect(created.ok()).toBeTruthy();
		const { id } = (await created.json()) as { id: string };
		await expect
			.poll(
				async () =>
					(
						(await (await api.get(`/api/admin/media/${id}`)).json()) as {
							status: string;
						}
					).status,
				{ timeout: 45_000 },
			)
			.toBe("ready");
		const asset = (await (await api.get(`/api/admin/media/${id}`)).json()) as {
			variants: Variant[];
		};
		const variant = largestVariant(asset.variants);
		const bytes = await (await api.get(variant.url)).body();
		await expectNoExif(bytes);
		await api.dispose();
	});

	test("heic upload shows indonesian error", async () => {
		await page.goto("/admin/desa/profil");
		await page.locator('input[type="file"]').last().setInputFiles({
			name: "foto.heic",
			mimeType: "image/heic",
			buffer: FAKE_HEIC,
		});
		await expect(
			page.getByRole("alert").filter({ hasText: HEIC_ERROR }),
		).toBeVisible();
	});

	test("an expired session returns to sign-in", async ({ browser }) => {
		const fresh = await browser.newContext({
			viewport: { width: 390, height: 844 },
		});
		const other = await fresh.newPage();
		await signIn(other);
		await other.goto("/admin/desa/profil");
		await other.getByRole("textbox", { name: "Gambaran umum" }).click();
		await other.keyboard.type(" x");
		// The server forgets the session (cookie gone), as after expiry.
		await fresh.clearCookies();
		await other.getByRole("button", { name: "Simpan", exact: true }).click();
		await expect(other).toHaveURL(/\/masuk\?ke=/);
		await expect(other.getByLabel("Nama pengguna")).toBeVisible();
		await fresh.close();
	});

	test("admin pages have no axe violations", async () => {
		for (const path of ["/admin", "/admin/desa/profil"]) {
			await page.goto(path);
			await expect(
				page.getByRole("heading", { level: 2 }).first(),
			).toBeVisible();
			const result = await analyzeAxe(page);
			expect(result.violations, path).toEqual([]);
		}
		for (const path of ["/masuk", "/akses"]) {
			const anonymous = await context.browser()?.newContext({
				ignoreHTTPSErrors: true,
				viewport: { width: 390, height: 844 },
			});
			const view = await anonymous?.newPage();
			if (!view || !anonymous) throw new Error("no browser");
			await view.goto(path);
			await view.waitForLoadState("networkidle");
			const result = await analyzeAxe(view);
			expect(result.violations, path).toEqual([]);
			await anonymous.close();
		}
	});
});
