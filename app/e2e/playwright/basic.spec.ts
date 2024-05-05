import { test, expect } from '@playwright/test';

test('has title GitButler', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await page.screenshot({ path: 'cdp.png' });

    await expect(page).toHaveTitle('');
});

test('has text package.json', async ({ page }) => {
    await page.goto('http://localhost:1420');
    const listBox = page.getByRole('listbox').getByRole('button')
    await expect(listBox).toHaveText('package.json');
})
