import { test, expect } from '@playwright/test';

test('has title', async ({ page }) => {
	await page.goto('/');

	await expect(page).toHaveTitle(/Home/);
});

test('downloads load', async ({ page }) => {
	await page.goto('/');

	await page.getByRole('link', { name: 'Downloads' }).click();

	await expect(page.getByRole('heading', { name: 'Stable Release' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Nightly Release' })).toBeVisible();
});
