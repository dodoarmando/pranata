import { expect, test } from "@playwright/test";
import { analyzeAxe } from "./axe";
import {
	PASSKEY_ADMIN_PASSWORD,
	PASSKEY_ADMIN_USERNAME,
	passkeyAdminAccessLink,
} from "./stack";

test.use({ viewport: { width: 1280, height: 800 } });

test("/masuk hydrates without warnings and offers the Passkey button", async ({
	page,
}) => {
	const messages: string[] = [];
	page.on("console", (message) => messages.push(message.text()));
	page.on("pageerror", (error) => messages.push(error.message));
	await page.goto("/masuk");
	await expect(page.getByRole("button", { name: "Passkey" })).toBeVisible();
	expect(messages.filter((m) => /hydrat/i.test(m))).toEqual([]);
});

test("add a passkey, sign out, sign in with it", async ({ page, context }) => {
	const cdp = await context.newCDPSession(page);
	await cdp.send("WebAuthn.enable");
	await cdp.send("WebAuthn.addVirtualAuthenticator", {
		options: {
			protocol: "ctap2",
			transport: "internal",
			hasResidentKey: true,
			hasUserVerification: true,
			isUserVerified: true,
			automaticPresenceSimulation: true,
		},
	});

	// Redeem the Access Link, then sign in with the new password.
	await page.goto(`/akses${new URL(passkeyAdminAccessLink()).hash}`);
	await page.getByLabel("Kata sandi baru").fill(PASSKEY_ADMIN_PASSWORD);
	await page.getByLabel("Ulangi kata sandi").fill(PASSKEY_ADMIN_PASSWORD);
	await page.getByRole("button", { name: "Simpan kata sandi" }).click();
	await expect(page).toHaveURL(/\/masuk$/);
	await page.getByLabel("Nama pengguna").fill(PASSKEY_ADMIN_USERNAME);
	await page
		.getByLabel("Kata sandi", { exact: true })
		.fill(PASSKEY_ADMIN_PASSWORD);
	await page.getByRole("button", { name: "Masuk", exact: true }).click();
	await expect(page).toHaveURL(/\/admin$/);

	// Add a passkey from the account menu (signing in opened Sudo Mode).
	await page.getByRole("button", { name: /Admin Sandi/ }).click();
	await page.getByRole("menuitem", { name: "Keamanan akun" }).click();
	await expect(page).toHaveURL(/\/admin\/akun$/);
	await expect(page.getByText(/^Belum ada passkey\./)).toBeVisible();
	await page.getByRole("button", { name: "Tambah passkey" }).click();
	await page.getByLabel("Nama passkey").fill("Kunci uji");
	await page.getByRole("button", { name: "Lanjutkan" }).click();
	const item = page.getByRole("listitem").filter({ hasText: "Kunci uji" });
	await expect(item).toContainText("Belum pernah dipakai");
	await expect(page.getByRole("dialog")).toBeHidden();

	const results = await analyzeAxe(page);
	expect(results.violations, "/admin/akun").toEqual([]);

	// Sign out and back in with the passkey, without typing a username.
	await page.getByRole("button", { name: /Admin Sandi/ }).click();
	await page.getByRole("menuitem", { name: "Keluar" }).click();
	await expect(page).toHaveURL(/\/masuk/);
	await page.getByRole("button", { name: "Passkey" }).click();
	await expect(page).toHaveURL(/\/admin$/);

	// Signing in with a passkey opens Sudo Mode too, so removing it goes through.
	await page.goto("/admin/akun");
	await expect(item).toContainText("Terakhir dipakai");
	await item.getByRole("button", { name: "Hapus" }).click();
	await expect(page.getByText(/^Belum ada passkey\./)).toBeVisible();
});
