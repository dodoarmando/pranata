import { expect, test } from '@playwright/test';

// What the composed home page shows depends on what the Village has published; the hero, with
// its search box, is always there.
test('the home page opens with the hero and its search box', async ({ page }) => {
  const response = await page.goto('/');
  expect(response?.status()).toBe(200);
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  const search = page.getByRole('search');
  await expect(search.getByRole('searchbox', { name: 'Cari' })).toBeVisible();
  await expect(search.getByRole('button', { name: 'Cari' })).toBeVisible();
});
