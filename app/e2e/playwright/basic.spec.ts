import { test, expect } from '@playwright/test';

test('has empty title', async ({ page }) => {
	await page.goto('http://localhost:1420');

	await expect(page).toHaveTitle('');
});

test('has text package.json', async ({ page }) => {
	await page.goto('http://localhost:1420');

	const listBox = page.getByRole('listbox').getByRole('button').first();

	await expect(listBox).toHaveText('package.json');
});
