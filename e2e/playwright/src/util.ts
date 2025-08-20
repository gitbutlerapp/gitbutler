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

/**
 * Click an element by test ID.
 */
export async function clickByTestId(page: Page, testId: TestIdValues): Promise<Locator> {
	const element = getByTestId(page, testId);
	await element.waitFor();
	await element.click();
	return element;
}
