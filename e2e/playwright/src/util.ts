import { TestId } from '@gitbutler/ui/utils/testIds';
import { type Locator, type Page } from '@playwright/test';

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
	await element.waitFor({ state: 'detached' });
}

/**
 * Click an element by test ID.
 */
export async function clickByTestId(
	page: Page,
	testId: TestIdValues,
	force?: boolean
): Promise<Locator> {
	const element = await waitForTestId(page, testId);
	await element.click({
		force
	});
	return element;
}

/**
 * Drag and drop an element onto another element by their test IDs.
 */
export async function dragAndDropByTestId(
	page: Page,
	sourceId: TestIdValues,
	targetId: TestIdValues
) {
	const source = await waitForTestId(page, sourceId);
	const target = await waitForTestId(page, targetId);

	await source.hover();
	await page.mouse.down();
	await target.hover();
	await target.hover({ force: true });
	await page.mouse.up();
}

/**
 * Drag and drop an element onto another element by their locators.
 */
export async function dragAndDropByLocator(page: Page, source: Locator, target: Locator) {
	await source.hover();
	await page.mouse.down();
	await target.hover();
	await target.hover({ force: true });
	await page.mouse.up();
}

export async function fillByTestId(
	page: Page,
	testId: TestIdValues,
	value: string
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
