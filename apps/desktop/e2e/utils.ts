import { spawnSync } from 'node:child_process';
import { browser } from '@wdio/globals';

const DEFAULT_TIMEOUT = 5_000;

export async function spawnAndLog(command: string, args: string[]) {
	const result = spawnSync(command, args);
	console.log(`Exec command: ${command} ${args.join(' ')}`);
	console.log('==== STDOUT ====\n', result.stdout?.toString());
	console.log('==== STDERR ====\n', result.stderr?.toString());
	console.log('==== EXIT STATUS ====\n', result.status);
	return result.status;
}

export async function findAndClick(selector: string, timeout?: number) {
	const button = await $(selector);
	await button.waitForClickable({
		timeout: timeout ?? DEFAULT_TIMEOUT
	});
	await browser.execute('arguments[0].click();', button);
}

export async function setElementValue(targetElement: WebdriverIO.Element, value: string) {
	await browser.execute(
		(input, path) => {
			(input as any).value = path;
		},
		targetElement,
		value
	);
}
