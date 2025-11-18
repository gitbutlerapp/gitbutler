import { sleep } from '$lib/utils/sleep';

/**
 * Tries to focus the claude input.
 *
 * This currently operates via a retried selector.
 */
export async function focusClaudeInput(stackId: string) {
	const element = await hackyRetriedSelector(`[data-id="${stackId}"] .ContentEditable__root`);
	if (element instanceof HTMLElement) {
		element?.focus();
	}
}

/**
 * Tries to find the element at a given selector with a time limit and a
 * refreshInterval.
 *
 * Retried selectors should be the _last_ resort. Prefer anything else.
 */
async function hackyRetriedSelector(
	selector: string,
	timeLimit = 50,
	refreshInterval = 1
): Promise<Element | undefined> {
	let tries = 0;
	while (refreshInterval * tries < timeLimit) {
		const element = document.querySelector(selector);
		if (element) return element;
		await sleep(refreshInterval);
		tries += 1;
	}
}
