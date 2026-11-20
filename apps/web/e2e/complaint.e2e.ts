import { expect, type Page, type Response, test } from '@playwright/test';

/** Neither page is indexed nor kept by a cache. */
async function expectPrivate(page: Page, response: Response | null) {
  expect(response?.status()).toBe(200);
  expect(response?.headers()['cache-control']).toBe('no-store');
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'noindex');
}

test('a resident sends a Complaint and reads its status by its code', async ({ page }) => {
  await expectPrivate(page, await page.goto('/pengaduan'));
  const submit = page.getByRole('button', { name: 'Kirim pengaduan' });
  // The proof of work is solved in the background; until then the button says so.
  await expect(submit).toBeEnabled({ timeout: 15_000 });

  await page.getByLabel('Topik').selectOption({ label: 'Infrastruktur' });
  await page.getByLabel('Isi pengaduan atau saran').fill('Lampu jalan di depan balai mati.');
  await submit.click();

  await expect(page.getByRole('heading', { name: 'Pengaduan terkirim' })).toBeVisible();
  const code = (await page.getByText(/^[0-9A-Z]{4}(-[0-9A-Z]{4}){3}$/).textContent()) ?? '';
  await page.getByRole('link', { name: 'Lihat status pengaduan' }).click();

  await expect(page.getByRole('heading', { name: 'Status pengaduan' })).toBeVisible();
  await expect(page.getByText(code)).toBeVisible();
  await expect(page.getByText('Infrastruktur')).toBeVisible();
  await expect(page.getByText('Diterima')).toBeVisible();
  await expect(page.getByText('Belum ada tanggapan.')).toBeVisible();

  // Opened directly, as from a saved link, in any case.
  await expectPrivate(page, await page.goto(`/pengaduan/status/${code.toLowerCase()}`));
  await expect(page.getByText('Diterima')).toBeVisible();
});

test('an unknown Complaint Code is a 404', async ({ page }) => {
  const response = await page.goto('/pengaduan/status/0000-0000-0000-0000');
  expect(response?.status()).toBe(404);
  await expect(page.getByRole('heading', { name: 'Pengaduan tidak ditemukan' })).toBeVisible();
});
