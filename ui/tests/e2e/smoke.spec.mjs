import { expect, test } from '@playwright/test';

async function ensureAuthenticated(page) {
  await page.goto('/auth');
  await page.waitForURL('**/index.html', { timeout: 15_000 });
}

test.describe('rust-mule ui smoke', () => {
  test('overview renders core sections', async ({ page }) => {
    await ensureAuthenticated(page);
    await expect(page.getByRole('heading', { name: /Search Overview/i })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Searches' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Nodes / Routing' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Logs' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Settings' })).toBeVisible();
  });

  test('search page has keyword form controls', async ({ page }) => {
    await ensureAuthenticated(page);
    await page.goto('/ui/search');
    await expect(page.getByRole('heading', { name: 'Keyword Search' })).toBeVisible();
    await expect(page.locator('#query')).toBeVisible();
    await expect(page.locator('#keyword-id-hex')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Search Keyword' })).toBeVisible();
  });

  test('node stats page has chart canvases and peers table', async ({ page }) => {
    await ensureAuthenticated(page);
    await page.goto('/ui/node_stats');
    await expect(page.getByRole('heading', { name: 'Node Stats' })).toBeVisible();
    await expect(page.locator('#hitsChart')).toBeVisible();
    await expect(page.locator('#rateChart')).toBeVisible();
    await expect(page.locator('#peersChart')).toBeVisible();
    await expect(page.getByRole('table')).toBeVisible();
  });

  test('logs and settings pages render key controls', async ({ page }) => {
    await ensureAuthenticated(page);
    await page.goto('/ui/log');
    await expect(page.getByRole('heading', { name: 'Logs' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Snapshot' })).toBeVisible();

    await page.goto('/ui/settings');
    await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Save Settings' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Rotate API Token' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Logout Session' })).toBeVisible();
  });
});
