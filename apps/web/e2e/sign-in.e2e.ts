import { expect, test } from '@playwright/test';

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
  await page.getByRole('button', { name: 'Masuk' }).click();
  await expect(page.getByRole('heading', { name: 'Beranda' })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Menu admin' })).toContainText('Log audit');

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
  await page.getByRole('button', { name: 'Masuk' }).click();
  await expect(page.getByRole('alert')).toHaveText('Nama pengguna atau kata sandi salah.');

  await page.getByText('Lupa kata sandi?').click();
  await expect(page.getByText(/Hubungi Administrator desa/)).toBeVisible();
});
