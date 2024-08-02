const DEFAULT_TIMEOUT = 5_000;

export async function findAndClick(selector, timeout) {
	const button = await $(selector);
	await button.waitForClickable({
		timeout: timeout ?? DEFAULT_TIMEOUT
	});
	await browser.execute('arguments[0].click();', button);
}
