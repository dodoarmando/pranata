import { expect, test } from '@playwright/test';

// `just e2e-run` provisions a desa code, so the chrome shows a Village and its region.
test('the header and footer show the Village name and region', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('banner')).toContainText(/Desa .+Kab\. .+, .+/);
  await expect(page.getByRole('contentinfo')).toContainText(/Pemerintah Desa .+Kec\. .+, Kab\. /);
});
