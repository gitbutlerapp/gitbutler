import { expect } from '@playwright/experimental-ct-svelte';

/**
 * Page type from Playwright test context
 * Using 'any' since @playwright/test types aren't available in component tests
 */
type Page = any;

/**
 * Wait for browser to be idle (no pending network requests, animations settled, etc.)
 */
export async function waitForBrowserIdle(
	page: Page,
	options: {
		timeout?: number;
		networkIdleTime?: number;
		checkAnimations?: boolean;
	} = {}
): Promise<void> {
	const { timeout = 5000, networkIdleTime = 500, checkAnimations = true } = options;

	// Wait for network to be idle
	try {
		await page.waitForLoadState('networkidle', { timeout: networkIdleTime });
	} catch {
		// Network idle timeout is acceptable - just means network settled quickly
	}

	// Additionally check for pending animations and other activity
	if (checkAnimations) {
		await page.evaluate(
			async (opts: { timeout: number; networkIdleTime: number }) => {
				// Wait for any pending animations to complete
				const animations = document.getAnimations();
				if (animations.length > 0) {
					await Promise.race([
						Promise.all(animations.map(async (anim) => await anim.finished)),
						new Promise((resolve) => setTimeout(resolve, opts.networkIdleTime))
					]);
				}

				// Wait for any pending microtasks
				await new Promise((resolve) => setTimeout(resolve, 0));
			},
			{ timeout, networkIdleTime }
		);
	}

	// Final small delay to ensure everything has settled
	await new Promise((resolve) => setTimeout(resolve, 50));
}

/**
 * Wait for a condition to be true, with automatic browser idle detection
 */
export async function waitForConditionWithIdle<T>(
	page: Page,
	getter: () => Promise<T>,
	matcher: (value: T) => boolean | Promise<boolean>,
	options: {
		timeout?: number;
		interval?: number;
		waitForIdle?: boolean;
		idleTimeout?: number;
	} = {}
): Promise<void> {
	const { timeout = 5000, interval = 100, waitForIdle = true, idleTimeout = 500 } = options;
	const startTime = Date.now();

	while (Date.now() - startTime < timeout) {
		// Wait for browser to be idle before checking condition
		if (waitForIdle) {
			try {
				await waitForBrowserIdle(page, { timeout: idleTimeout, networkIdleTime: 200 });
			} catch {
				// Continue even if idle detection times out
			}
		}

		// Check the condition
		const value = await getter();
		if (await matcher(value)) {
			return;
		}

		// Wait before next check
		await new Promise((resolve) => setTimeout(resolve, interval));
	}

	// One final check after waiting for idle
	if (waitForIdle) {
		await waitForBrowserIdle(page, { timeout: idleTimeout, networkIdleTime: 200 });
	}

	const finalValue = await getter();
	if (await matcher(finalValue)) {
		return;
	}

	throw new Error(
		`waitForConditionWithIdle timed out after ${timeout}ms. Last value: ${JSON.stringify(finalValue)}`
	);
}

/**
 * Generic helper to wait for a value from a getter function to match an expectation
 * @deprecated Use waitForConditionWithIdle for better idle detection
 */
export async function waitFor<T>(
	getter: () => Promise<T>,
	matcher: (value: T) => boolean | Promise<boolean>,
	options: { timeout?: number; interval?: number } = {}
): Promise<void> {
	const { timeout = 2000, interval = 50 } = options;
	const startTime = Date.now();

	while (Date.now() - startTime < timeout) {
		const value = await getter();
		if (await matcher(value)) {
			return;
		}
		await new Promise((resolve) => setTimeout(resolve, interval));
	}

	throw new Error(`waitFor timed out after ${timeout}ms`);
}

/**
 * Wait for text content of a test element to contain expected text
 * Uses expect.poll which has built-in retry logic
 */
export async function waitForTextContent(
	component: any,
	expectedText: string,
	timeout = 2000
): Promise<void> {
	await expect
		.poll(async () => (await component.getByTestId('text-content').textContent()) || '', {
			timeout
		})
		.toContain(expectedText);
}

/**
 * Wait for text content with browser idle detection
 * More robust than waitForTextContent for complex UI updates
 */
export async function waitForTextContentWithIdle(
	page: Page,
	component: any,
	expectedText: string,
	options: { timeout?: number; idleFirst?: boolean } = {}
): Promise<void> {
	const { timeout = 5000, idleFirst = true } = options;

	// Optionally wait for idle state first
	if (idleFirst) {
		await waitForBrowserIdle(page, { timeout: 1000 });
	}

	await waitForConditionWithIdle(
		page,
		async () => (await component.getByTestId('text-content').textContent()) || '',
		(text) => text.includes(expectedText),
		{ timeout }
	);
}

/**
 * Wait for a test data attribute to match a specific value
 */
export async function waitForTestId(
	component: any,
	testId: string,
	expectedValue: string | number,
	timeout = 2000
): Promise<void> {
	await expect
		.poll(async () => (await component.getByTestId(testId).textContent()) || '', { timeout })
		.toBe(String(expectedValue));
}

/**
 * Wait for a condition to be true using expect.poll
 */
export async function waitForCondition(
	condition: () => Promise<boolean>,
	timeout = 2000
): Promise<void> {
	await expect.poll(condition, { timeout }).toBe(true);
}

/**
 * Get text content from a component's test-content element
 */
export async function getTextContent(component: any): Promise<string> {
	return (await component.getByTestId('text-content').textContent()) || '';
}

/**
 * Get numeric value from a test element (e.g., paragraph-count)
 */
export async function getTestIdValue(component: any, testId: string): Promise<number> {
	const text = await component.getByTestId(testId).textContent();
	return parseInt(text || '0', 10);
}

/**
 * Wait for scroll position to stabilize (useful for auto-scroll features)
 */
export async function waitForScrollStability(
	viewport: any,
	options: {
		timeout?: number;
		requiredStableChecks?: number;
		checkInterval?: number;
	} = {}
): Promise<void> {
	const { timeout = 1000, requiredStableChecks = 3, checkInterval = 50 } = options;

	await viewport.evaluate(
		async (
			el: HTMLElement,
			opts: { timeout: number; requiredStableChecks: number; checkInterval: number }
		) => {
			return await new Promise<void>((resolve) => {
				let lastScrollTop = el.scrollTop;
				let stableCount = 0;

				const interval = setInterval(() => {
					if (el.scrollTop === lastScrollTop) {
						stableCount++;
						if (stableCount >= opts.requiredStableChecks) {
							clearInterval(interval);
							resolve();
						}
					} else {
						stableCount = 0;
						lastScrollTop = el.scrollTop;
					}
				}, opts.checkInterval);

				// Safety timeout
				setTimeout(() => {
					clearInterval(interval);
					resolve();
				}, opts.timeout);
			});
		},
		{ timeout, requiredStableChecks, checkInterval }
	);
}

/**
 * Wait for a DOM property to change beyond a threshold
 */
export async function waitForPropertyChange<T extends number>(
	element: any,
	propertyGetter: (el: HTMLElement) => T,
	comparison: (newValue: T, oldValue: T) => boolean,
	oldValue: T,
	timeout = 2000
): Promise<void> {
	await element.evaluate(
		async (
			el: HTMLElement,
			args: {
				propertyGetter: string;
				comparison: string;
				oldValue: number;
				timeout: number;
			}
		) => {
			const getter = new Function('el', `return ${args.propertyGetter}`) as (
				el: HTMLElement
			) => number;
			const compare = new Function('newVal', 'oldVal', `return ${args.comparison}`) as (
				newVal: number,
				oldVal: number
			) => boolean;

			return await new Promise<void>((resolve) => {
				function check() {
					if (compare(getter(el), args.oldValue)) {
						resolve();
					} else {
						setTimeout(check, 50);
					}
				}
				check();
				setTimeout(resolve, args.timeout);
			});
		},
		{
			propertyGetter: propertyGetter.toString(),
			comparison: comparison.toString(),
			oldValue,
			timeout
		}
	);
}

/**
 * Get scroll-related properties from a scrollable element
 */
export async function getScrollProperties(viewport: any): Promise<{
	scrollTop: number;
	scrollHeight: number;
	clientHeight: number;
}> {
	return await viewport.evaluate((el: HTMLElement) => ({
		scrollTop: el.scrollTop,
		scrollHeight: el.scrollHeight,
		clientHeight: el.clientHeight
	}));
}

/**
 * Get distance from bottom of scroll container
 */
export async function getDistanceFromBottom(viewport: any): Promise<number> {
	return await viewport.evaluate((el: HTMLElement) => {
		return el.scrollHeight - el.scrollTop - el.clientHeight;
	});
}

/**
 * Scroll element to a specific position
 */
export async function scrollTo(viewport: any, scrollTop: number): Promise<void> {
	await viewport.evaluate((el: HTMLElement, top: number) => {
		el.scrollTop = top;
	}, scrollTop);
}

/**
 * Wait for scrollHeight to increase beyond a threshold
 */
export async function waitForScrollHeightIncrease(
	viewport: any,
	oldHeight: number,
	timeout = 2000
): Promise<void> {
	await viewport.evaluate(
		async (el: HTMLElement, oldHeight: number, timeout: number) => {
			return await new Promise<void>((resolve) => {
				function checkHeight() {
					if (el.scrollHeight > oldHeight) {
						resolve();
					} else {
						setTimeout(checkHeight, 50);
					}
				}
				checkHeight();
				setTimeout(resolve, timeout);
			});
		},
		oldHeight,
		timeout
	);
}

/**
 * Advanced browser activity detector
 * Monitors multiple signals to determine if browser is truly idle
 */
export async function isBrowserBusy(page: Page): Promise<boolean> {
	return await page.evaluate(() => {
		// Check 1: Pending network requests (if Performance API is available)
		if (typeof performance !== 'undefined' && performance.getEntriesByType) {
			const resources = performance.getEntriesByType('resource') as PerformanceResourceTiming[];
			const recentResources = resources.filter((r) => {
				const age = performance.now() - r.startTime;
				return age < 1000 && r.responseEnd === 0; // Started recently and not yet complete
			});
			if (recentResources.length > 0) return true;
		}

		// Check 2: Running animations
		const animations = document.getAnimations();
		const activeAnimations = animations.filter((anim) => anim.playState === 'running');
		if (activeAnimations.length > 0) return true;

		// Check 3: Pending transitions
		const allElements = document.querySelectorAll('*');
		for (const el of Array.from(allElements)) {
			const styles = window.getComputedStyle(el as Element);
			const transitionDuration = styles.transitionDuration;
			if (transitionDuration && transitionDuration !== '0s') {
				// Element might be transitioning
				return true;
			}
		}

		// Check 4: Pending RAF callbacks (heuristic - check if RAF is being called frequently)
		// This is done by scheduling a RAF and seeing if it takes a while
		// (implemented via a promise that resolves in the caller)

		return false;
	});
}

/**
 * Wait until browser is truly idle by continuously checking activity
 */
export async function waitUntilIdle(
	page: Page,
	options: {
		maxWait?: number;
		checkInterval?: number;
		consecutiveIdleChecks?: number;
	} = {}
): Promise<void> {
	const { maxWait = 5000, checkInterval = 50, consecutiveIdleChecks = 3 } = options;

	const startTime = Date.now();
	let idleCount = 0;

	while (Date.now() - startTime < maxWait) {
		const busy = await isBrowserBusy(page);

		if (!busy) {
			idleCount++;
			if (idleCount >= consecutiveIdleChecks) {
				// Browser has been idle for required number of checks
				return;
			}
		} else {
			idleCount = 0; // Reset counter if browser becomes busy again
		}

		await new Promise((resolve) => setTimeout(resolve, checkInterval));
	}

	// Timeout reached - browser might still be busy but we've waited long enough
	// This is acceptable for test stability
}

/**
 * Execute an action and wait for browser to become idle afterwards
 * Useful for user interactions like typing, clicking, etc.
 */
export async function doAndWaitForIdle(
	page: Page,
	action: () => Promise<void>,
	options: {
		maxWait?: number;
		checkInterval?: number;
		consecutiveIdleChecks?: number;
	} = {}
): Promise<void> {
	await action();
	await waitUntilIdle(page, options);
}
