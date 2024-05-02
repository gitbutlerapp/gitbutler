import { test, expect } from '@playwright/test';
// import { getUtils } from '../utils';
// import { mockIPC } from '@tauri-apps/api/mocks';

// let utils;

// test.beforeEach(({ context, page }) => {
// 	utils = getUtils(page);
// });
//
// test.beforeEach(async ({ page, context }) => {
// 	context.exposeFunction('__TAURI_IPC__', (data) => {
// 		console.log('ipc', data);
// 	});
// });

test('has title GitButler', async ({ page }) => {
	// mockIPC((cmd, args) => {
	// 	// simulated rust command called "add" that just adds two numbers
	// 	if (cmd === 'add') {
	// 		return (args.a as number) + (args.b as number);
	// 	}
	// });
	await page.goto('http://localhost:1420');
	await page.screenshot({ path: 'cdp.png' });

	// Expect a title "to contain" a substring.
	await expect(page).toHaveTitle('GitButler Dev');
});

// import playwright from 'playwright';
//
// const browser = await playwright.chromium.connectOverCDP(
// 	'ws://localhost:9222'
// 	// 'ws://localhost:9222/devtools/page/localhost:1420'
// 	// 'ws://localhost:9222/devtools/page'
// );
//
// const context = await browser.newContext();
// const page = await context.newPage();
//
// await page.goto('http://localhost:1420/');
// await page.screenshot({ path: 'cdp.png' });
//
// await browser.close();
