/* eslint-disable no-console */
import { spawnSync } from "node:child_process";

export async function spawnAndLog(command: string, args: string[]) {
	const result = spawnSync(command, args);
	console.log(`Exec command: ${command} ${args.join(" ")}`);
	console.log("==== STDOUT ====\n", result.stdout?.toString());
	console.log("==== STDERR ====\n", result.stderr?.toString());
	console.log("==== EXIT STATUS ====\n", result.status);
	return result.status;
}

export async function findAndClick(selector: string) {
	const element = $(selector);
	await element.waitForClickable({ timeout: 30000 });
	await element.click();
}

export async function findElement(selector: string) {
	const element = $(selector);
	await element.waitForExist({ timeout: 30000 });
	return await element.getElement();
}

export async function setElementValue(targetElement: WebdriverIO.Element, value: string) {
	await browser.execute(
		(input, path) => {
			(input as any).value = path;
		},
		targetElement,
		value,
	);
}
