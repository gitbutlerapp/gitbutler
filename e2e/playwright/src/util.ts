import { TestId } from "@gitbutler/ui/utils/testIds";
import { type Locator, type Page } from "@playwright/test";

type TestIdValues = `${TestId}`;

/**
 * Get by test ID from the page.
 *
 * This is only here in order to have nice autocompletion in the IDE.
 */
export function getByTestId(page: Page, testId: TestIdValues) {
	return page.getByTestId(testId);
}

export async function waitForTestId(page: Page, testId: TestIdValues): Promise<Locator> {
	const element = getByTestId(page, testId);
	await element.waitFor();
	return element;
}

export async function waitForTestIdToNotExist(page: Page, testId: TestIdValues): Promise<void> {
	const element = getByTestId(page, testId);
	await element.waitFor({ state: "detached" });
}

/**
 * Click an element by test ID.
 */
export async function clickByTestId(
	page: Page,
	testId: TestIdValues,
	force?: boolean,
): Promise<Locator> {
	const element = await waitForTestId(page, testId);
	await element.click({
		force,
	});
	return element;
}

export async function rightClickByTestId(page: Page, testId: TestIdValues): Promise<Locator> {
	const element = await waitForTestId(page, testId);
	await element.click({
		button: "right",
	});
	return element;
}

/**
 * Drag and drop an element onto another element by their test IDs.
 */
export async function dragAndDropByTestId(
	page: Page,
	sourceId: TestIdValues,
	targetId: TestIdValues,
) {
	const source = await waitForTestId(page, sourceId);
	const target = await waitForTestId(page, targetId);

	await source.hover();
	await page.mouse.down();
	await target.hover();
	await target.hover({ force: true });
	await page.mouse.up();
}

type DropOptions = {
	force?: boolean;
	position?: {
		x: number;
		y: number;
	};
};

/**
 * Drag and drop an element onto another element by their locators.
 */
export async function dragAndDropByLocator(
	page: Page,
	source: Locator,
	target: Locator,
	options: DropOptions = {},
) {
	await source.hover();
	await page.mouse.down();
	// Always wait a bit in case CSS causes content shift.
	await page.waitForTimeout(100);
	await target.hover({ force: options.force, position: options.position });
	// The drag system uses requestAnimationFrame to detect dropzones via
	// document.elementFromPoint. Wait for at least one animation frame so the
	// dropzone is detected as hovered before we release the mouse button.
	// eslint-disable-next-line @typescript-eslint/promise-function-async
	await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));
	await page.mouse.up();
}

export async function fillByTestId(
	page: Page,
	testId: TestIdValues,
	value: string,
): Promise<Locator> {
	const element = await waitForTestId(page, testId);
	await element.fill(value);
	return element;
}

/**
 * Type into the rich text editor by test ID.
 *
 * Only use this for the rich text editor, as this is a workaround for the fact that
 * the rich text editor does not support the `fill` method.
 *
 * If you need to pass text into a norma input element, @see fillByTestId instead
 */
export async function textEditorFillByTestId(page: Page, testId: TestIdValues, value: string) {
	const element = await waitForTestId(page, testId);
	await element.click();
	await element.pressSequentially(value);
	return element;
}

export async function sleep(ms: number): Promise<void> {
	return await new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Wait until an element's bounding box stops changing between animation frames.
 * Useful for popups positioned by Floating UI that take a few frames to settle.
 */
export async function waitForElementToStabilize(page: Page, locator: Locator, timeout = 5000) {
	const start = Date.now();
	let lastBox = await locator.boundingBox();
	while (Date.now() - start < timeout) {
		// eslint-disable-next-line @typescript-eslint/promise-function-async
		await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => r())));
		const box = await locator.boundingBox();
		if (
			box &&
			lastBox &&
			Math.abs(box.x - lastBox.x) < 1 &&
			Math.abs(box.y - lastBox.y) < 1 &&
			Math.abs(box.width - lastBox.width) < 1 &&
			Math.abs(box.height - lastBox.height) < 1
		) {
			return;
		}
		lastBox = box;
	}
	throw new Error(
		`Element did not stabilize within ${timeout}ms — last bounding box: ${JSON.stringify(lastBox)}`,
	);
}

/**
 * Mock the backend's native directory picker to return a specific path.
 *
 * The web frontend calls `POST /pick_directory` to open a native OS file dialog.
 * In e2e tests we intercept this request and return the desired path directly.
 * Must be called before the action that triggers the picker.
 */
export async function mockPickDirectory(page: Page, directoryPath: string): Promise<void> {
	await page.unroute("**/pick_directory");
	await page.route("**/pick_directory", async (route) => {
		await route.fulfill({
			status: 200,
			contentType: "application/json",
			body: JSON.stringify({ type: "success", subject: { path: directoryPath } }),
		});
	});
}
