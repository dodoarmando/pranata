import AxeBuilder from "@axe-core/playwright";
import {
	type APIRequestContext,
	type Browser,
	type BrowserContext,
	expect,
	type Page,
	request,
	test,
} from "@playwright/test";
import {
	ORIGIN,
	USERS_ADMIN_PASSWORD,
	USERS_ADMIN_USERNAME,
	usersAdminAccessLink,
} from "./stack";

const AXE_TAGS = ["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"];
const PASSWORD = "neng-password-123";
const NO_ACCESS = "Anda tidak memiliki hak akses untuk halaman ini.";

test.use({ viewport: { width: 390, height: 844 } });
test.describe.configure({ mode: "serial" });

async function newContext(browser: Browser): Promise<BrowserContext> {
	return browser.newContext({
		ignoreHTTPSErrors: true,
		viewport: { width: 390, height: 844 },
	});
}

// Opens an Access Link, sets the password and signs in.
async function redeemAndSignIn(
	page: Page,
	link: string,
	username: string,
	password: string,
) {
	await page.goto(`/akses${new URL(link).hash}`);
	await page.getByLabel("Kata sandi baru").fill(password);
	await page.getByLabel("Ulangi kata sandi").fill(password);
	await page.getByRole("button", { name: "Simpan kata sandi" }).click();
	await expect(page).toHaveURL(/\/masuk$/);
	await page.getByLabel("Nama pengguna").fill(username);
	await page.getByLabel("Kata sandi", { exact: true }).fill(password);
	await page.getByRole("button", { name: "Masuk", exact: true }).click();
	await expect(page).toHaveURL(/\/admin$/);
}

// The Administrator's own API session, used only to prepare data the screens
// under test need.
async function adminApi(): Promise<{
	api: APIRequestContext;
	post: (path: string, data: unknown) => Promise<unknown>;
}> {
	const api = await request.newContext({
		baseURL: ORIGIN,
		ignoreHTTPSErrors: true,
	});
	const login = await api.post("/api/auth/login", {
		data: { username: USERS_ADMIN_USERNAME, password: USERS_ADMIN_PASSWORD },
	});
	expect(login.ok()).toBeTruthy();
	const { csrf_token: csrf } = (await login.json()) as { csrf_token: string };
	const headers = { "x-csrf-token": csrf };
	const sudo = await api.post("/api/auth/sudo", {
		headers,
		data: { password: USERS_ADMIN_PASSWORD },
	});
	expect(sudo.ok()).toBeTruthy();
	return {
		api,
		post: async (path, data) => {
			const response = await api.post(path, { headers, data });
			expect(response.ok(), path).toBeTruthy();
			return response.json();
		},
	};
}

async function readLinkDialog(page: Page): Promise<string> {
	const dialog = page.getByRole("dialog");
	await expect(
		dialog.getByRole("heading", { name: /^Tautan akses untuk / }),
	).toBeVisible();
	return dialog.getByLabel("Tautan akses", { exact: true }).inputValue();
}

test.describe("users and access links", () => {
	let admin: BrowserContext;
	let adminPage: Page;
	let nengContext: BrowserContext;
	let nengPage: Page;

	test.beforeAll(async ({ browser }) => {
		admin = await newContext(browser);
		adminPage = await admin.newPage();
		await redeemAndSignIn(
			adminPage,
			usersAdminAccessLink(),
			USERS_ADMIN_USERNAME,
			USERS_ADMIN_PASSWORD,
		);
	});

	test.afterAll(async () => {
		await admin.close();
		await nengContext?.close();
	});

	test("administrator creates an editor role and a user, the user signs in", async ({
		browser,
	}) => {
		await adminPage.goto("/admin/sistem/peran");
		await expect(
			adminPage.getByRole("heading", { name: "Peran", level: 2 }),
		).toBeVisible();
		await adminPage.getByRole("button", { name: "Tambah peran" }).click();
		await adminPage.getByLabel("Nama peran").fill("Kontributor Web");
		await adminPage.getByRole("checkbox", { name: /Menulis konten/ }).check();
		await adminPage
			.getByRole("button", { name: "Simpan", exact: true })
			.click();
		await expect(
			adminPage.getByRole("button", { name: /Kontributor Web/ }),
		).toContainText("0 pengguna · 1 hak akses");

		await adminPage.goto("/admin/sistem/pengguna");
		await expect(
			adminPage.getByRole("heading", { name: "Pengguna", level: 2 }),
		).toBeVisible();
		await adminPage.getByRole("button", { name: "Tambah pengguna" }).click();
		await adminPage.getByLabel("Nama lengkap").fill("Neng Sari");
		await adminPage.getByLabel("Nama pengguna").fill("neng");
		await adminPage.getByLabel("Peran").click();
		await adminPage.getByRole("option", { name: "Kontributor Web" }).click();
		await adminPage
			.getByRole("button", { name: "Simpan", exact: true })
			.click();

		// Signing in opened the Sudo Mode window, so no password prompt here.
		const link = await readLinkDialog(adminPage);
		expect(link).toContain("/akses#");
		await adminPage.getByRole("button", { name: "Selesai" }).click();
		await expect(
			adminPage.getByRole("row", { name: /Neng Sari/ }),
		).toContainText("Kontributor Web");

		nengContext = await newContext(browser);
		nengPage = await nengContext.newPage();
		await redeemAndSignIn(nengPage, link, "neng", PASSWORD);
		await expect(nengPage.getByText("Sistem", { exact: true })).toHaveCount(0);
		await nengPage.goto("/admin/desa/profil");
		await expect(nengPage.getByText(NO_ACCESS)).toBeVisible();
	});

	test("deactivation signs the user out", async () => {
		await adminPage.goto("/admin/sistem/pengguna");
		await adminPage
			.getByRole("button", { name: "Menu untuk Neng Sari" })
			.click();
		await adminPage.getByRole("menuitem", { name: "Nonaktifkan" }).click();
		await expect(
			adminPage.getByText(
				"Nonaktifkan Neng Sari? Sesi masuknya berakhir dan tautan aksesnya tidak berlaku lagi.",
			),
		).toBeVisible();
		await adminPage
			.getByRole("dialog")
			.getByRole("button", { name: "Nonaktifkan" })
			.click();
		await expect(
			adminPage.getByRole("row", { name: /Neng Sari/ }),
		).toContainText("Nonaktif");

		await nengPage.goto("/admin");
		await expect(nengPage).toHaveURL(/\/masuk/);
	});

	test("role in use cannot be deleted", async () => {
		await adminPage.goto("/admin/sistem/peran");
		await adminPage.getByRole("button", { name: /Kontributor Web/ }).click();
		await adminPage.getByRole("button", { name: "Hapus", exact: true }).click();
		await adminPage
			.getByRole("dialog")
			.getByRole("button", { name: "Hapus", exact: true })
			.click();
		await expect(
			adminPage.getByText(
				"Peran masih dipakai pengguna, jadi tidak dapat dihapus.",
			),
		).toBeVisible();
	});

	test("a user manager is not offered the Administrator role", async ({
		browser,
	}) => {
		const { api, post } = await adminApi();
		await post("/api/admin/roles", {
			name: "Pengelola Pengguna",
			permissions: ["user.manage"],
		});
		const roles = (await (await api.get("/api/admin/roles")).json()) as {
			id: string;
			name: string;
		}[];
		const role = roles.find((r) => r.name === "Pengelola Pengguna");
		const created = (await post("/api/admin/users", {
			display_name: "Pengelola",
			username: "pengelola",
			role_id: role?.id,
		})) as { access_link: { url: string } };
		await api.dispose();

		const context = await newContext(browser);
		const page = await context.newPage();
		await redeemAndSignIn(page, created.access_link.url, "pengelola", PASSWORD);
		await page.goto("/admin/sistem/pengguna");
		await page.getByRole("button", { name: "Tambah pengguna" }).click();
		await page.getByLabel("Peran").click();
		await expect(
			page.getByRole("option", { name: "Pengelola Pengguna" }),
		).toBeVisible();
		await expect(
			page.getByRole("option", { name: "Administrator" }),
		).toHaveCount(0);
		await expect(
			page.getByRole("option", { name: "Kontributor Web" }),
		).toHaveCount(0);
		await context.close();
	});

	test("the roles screen has no axe violations", async () => {
		await adminPage.goto("/admin/sistem/peran");
		await adminPage.getByRole("button", { name: /Kontributor Web/ }).click();
		await expect(adminPage.getByLabel("Nama peran")).toHaveValue(
			"Kontributor Web",
		);
		const results = await new AxeBuilder({ page: adminPage })
			.withTags(AXE_TAGS)
			.analyze();
		expect(results.violations, "/admin/sistem/peran").toEqual([]);
	});

	test("the users screen and the access link dialog have no axe violations", async () => {
		await adminPage.goto("/admin/sistem/pengguna");
		await expect(
			adminPage.getByRole("row", { name: /Pengelola/ }),
		).toBeVisible();
		const screen = await new AxeBuilder({ page: adminPage })
			.withTags(AXE_TAGS)
			.analyze();
		expect(screen.violations, "/admin/sistem/pengguna").toEqual([]);

		await adminPage
			.getByRole("button", { name: "Menu untuk Pengelola" })
			.click();
		await adminPage
			.getByRole("menuitem", { name: "Buat tautan akses" })
			.click();
		await readLinkDialog(adminPage);
		const dialog = await new AxeBuilder({ page: adminPage })
			.withTags(AXE_TAGS)
			.analyze();
		expect(dialog.violations, "access link dialog").toEqual([]);
	});
});
