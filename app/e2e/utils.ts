import { Page } from '@playwright/test';

async function openDebugPanel(page: Page) {
    const isOpen = (await page.locator('[data-testid="debug-panel"]')?.getAttribute('open')) === '';

    if (!isOpen) {
        await page.getByText('Debug').click();
        await page.getByTestId('debug-panel').and(page.locator('[open]')).waitFor();
    }
}

async function closeDebugPanel(page: Page) {
    const isOpen = (await page.getByTestId('debug-panel')?.getAttribute('open')) === '';
    if (isOpen) {
        await page.getByText('Debug').click();
        await page.getByTestId('debug-panel').and(page.locator(':not([open])')).waitFor();
    }
}

export function getUtils(page: Page) {
    return {
        openDebugPanel: () => openDebugPanel(page),
        closeDebugPane: () => closeDebugPanel(page)
    };
}
