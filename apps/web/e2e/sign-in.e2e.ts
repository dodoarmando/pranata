import { expect, type Page, test } from '@playwright/test';

// `just e2e-run` creates an Administrator from the CLI and passes on the username and link.
const username = process.env.PRANATA_E2E_USERNAME ?? '';
const accessLink = process.env.PRANATA_E2E_ACCESS_LINK ?? '';
const password = 'kopi tubruk pagi hari';

test('the first Administrator sets a password from the Access Link, signs in and signs out', async ({
  page,
}) => {
  expect(username && accessLink, 'PRANATA_E2E_USERNAME and _ACCESS_LINK').toBeTruthy();
  await page.goto(accessLink);
  await page.getByLabel('Kata sandi baru').fill(password);
  await page.getByRole('button', { name: 'Simpan kata sandi' }).click();
  await expect(page.getByRole('heading', { name: 'Kata sandi tersimpan' })).toBeVisible();

  await page.getByRole('link', { name: 'masuk' }).click();
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Menu admin' })).toContainText('Log audit');

  await page.getByText('Sistem').click();
  await page.getByRole('link', { name: 'Log audit' }).click();
  await expect(page.getByRole('heading', { name: 'Log audit' })).toBeVisible();
  await page.getByLabel('Pengguna').fill(username);
  await page.getByRole('button', { name: 'Saring' }).click();
  await expect(page).toHaveURL(new RegExp(`username=${username}`));
  await expect(page.getByRole('table')).toContainText('session.sign_in');
  await expect(page.getByRole('table')).toContainText('access_link.use');
  await expect(page.getByRole('table')).toContainText('{"remember":false}');

  await page.getByRole('button', { name: 'Keluar' }).click();
  await expect(page).toHaveURL(/\/masuk$/);
  await page.goto('/admin');
  await expect(page).toHaveURL(/\/masuk$/);
});

test('a wrong password is refused and the forgotten-password hint names the Administrator', async ({
  page,
}) => {
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill('bukan kata sandinya');
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveText('Nama pengguna atau kata sandi salah.');

  await page.getByText('Lupa kata sandi?').click();
  await expect(page.getByText(/Hubungi Administrator desa/)).toBeVisible();
});

test('the Administrator creates a User from an Access Link, composes a Role and deactivates the User', async ({
  page,
  browser,
}) => {
  const newcomer = `${username}k`;
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Pengguna aktif' })).toBeVisible();

  await page.goto('/admin/pengguna');
  await page.getByLabel('Nama pengguna').fill(newcomer);
  await page
    .getByRole('combobox', { name: 'Peran', exact: true })
    .selectOption({ label: 'Kontributor Web' });
  await page.getByRole('button', { name: 'Buat pengguna' }).click();
  const link = page.getByRole('textbox', { name: 'Tautan Akses' });
  await expect(link).toHaveValue(/\/akses#[0-9a-f]{64}$/);

  const newcomerPage = await (await browser.newContext()).newPage();
  await newcomerPage.goto(await link.inputValue());
  await newcomerPage.getByLabel('Kata sandi baru').fill(password);
  await newcomerPage.getByRole('button', { name: 'Simpan kata sandi' }).click();
  await expect(newcomerPage.getByRole('heading', { name: 'Kata sandi tersimpan' })).toBeVisible();

  const row = page.getByRole('row').filter({ hasText: newcomer });
  await expect(row.getByRole('combobox')).toHaveValue(/.+/);
  page.once('dialog', (dialog) => dialog.accept());
  await row.getByRole('button', { name: 'Nonaktifkan' }).click();
  await expect(row).toContainText('Nonaktif');
  await expect(row.getByRole('button', { name: 'Aktifkan' })).toBeVisible();

  await page.goto('/admin/peran');
  const newRole = page.getByRole('form', { name: 'Peran baru' });
  await newRole.getByLabel('Nama peran').fill(`Peran ${username}`);
  await newRole.getByLabel('Baca pengaduan').check();
  await newRole.getByRole('button', { name: 'Buat peran' }).click();
  const saved = page.getByRole('form', { name: `Peran ${username}` });
  await expect(saved.getByLabel('Baca pengaduan')).toBeChecked();
  await expect(saved.getByLabel('Baca log audit')).not.toBeChecked();
});

/** A platform authenticator that verifies the User without asking, as Chrome's DevTools offer. */
async function addVirtualAuthenticator(page: Page) {
  const devtools = await page.context().newCDPSession(page);
  await devtools.send('WebAuthn.enable');
  await devtools.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });
}

test('a User adds a passkey on Akun saya and signs in with it instead of the password', async ({
  page,
}) => {
  await addVirtualAuthenticator(page);
  await page.goto('/masuk');
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByLabel('Kata sandi', { exact: true }).fill(password);
  await page.getByRole('button', { name: 'Masuk', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  await page.goto('/admin/akun');
  await page.getByLabel('Nama passkey').fill('Laptop uji');
  await page.getByRole('button', { name: 'Tambah passkey' }).click();
  await expect(page.getByRole('alert')).toHaveText('Passkey Laptop uji ditambahkan.');
  await expect(page.getByRole('cell', { name: 'Laptop uji', exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Keluar', exact: true }).click();
  await expect(page).toHaveURL(/\/masuk$/);
  await page.getByLabel('Nama pengguna').fill(username);
  await page.getByRole('button', { name: 'Masuk dengan passkey' }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();

  await page.goto(`/admin/log-audit?username=${username}`);
  await expect(page.getByRole('table')).toContainText('passkey.create');
  await expect(page.getByRole('table')).toContainText('"passkey":"Laptop uji"');
});
