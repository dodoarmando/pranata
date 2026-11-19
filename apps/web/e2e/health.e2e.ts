import { expect, test } from '@playwright/test';

test('the home page shows a healthy API', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('Status API: sehat')).toBeVisible();
});
