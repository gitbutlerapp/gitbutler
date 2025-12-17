import VirtualListTestWrapper from './VirtualListTestWrapper.svelte';
import { test, expect } from '@playwright/experimental-ct-svelte';

/**
 * Helper function to get distance from bottom of scroll container
 */
async function getDistanceFromBottom(viewport: any): Promise<number> {
	return await viewport.evaluate((el: HTMLElement) => {
		return el.scrollHeight - el.scrollTop - el.clientHeight;
	});
}

/**
 * Helper function to wait for scroll to stabilize
 */
async function waitForScrollStability(viewport: any, timeoutMs = 1000): Promise<void> {
	await viewport.evaluate(async (el: HTMLElement, timeout: number) => {
		return await new Promise<void>((resolve) => {
			let lastScrollTop = el.scrollTop;
			let stableCount = 0;
			const requiredStableChecks = 3;
			const checkInterval = 50;

			const interval = setInterval(() => {
				if (el.scrollTop === lastScrollTop) {
					stableCount++;
					if (stableCount >= requiredStableChecks) {
						clearInterval(interval);
						resolve();
					}
				} else {
					stableCount = 0;
					lastScrollTop = el.scrollTop;
				}
			}, checkInterval);

			// Safety timeout
			setTimeout(() => {
				clearInterval(interval);
				resolve();
			}, timeout);
		});
	}, timeoutMs);
}

/**
 * Helper function to wait for scrollHeight to increase beyond a threshold
 */
async function waitForScrollHeightIncrease(
	viewport: any,
	oldHeight: number,
	timeoutMs = 2000
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
				// Safety timeout
				setTimeout(resolve, timeout);
			});
		},
		oldHeight,
		timeoutMs
	);
}

/**
 * Helper function to get scroll properties
 */
async function getScrollProperties(viewport: any): Promise<{
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
 * Helper function to scroll to a specific position
 */
async function scrollTo(viewport: any, scrollTop: number): Promise<void> {
	await viewport.evaluate((el: HTMLElement, position: number) => {
		el.scrollTop = position;
	}, scrollTop);
}

// Test matrix: different combinations of itemCount and batchSize
const testConfigurations = [
	{ itemCount: 20, batchSize: 1, description: 'small list, batch size 1' },
	{ itemCount: 50, batchSize: 5, description: 'medium list, batch size 5' },
	{ itemCount: 100, batchSize: 10, description: 'large list, batch size 10' }
];

// Run tests for each configuration
for (const config of testConfigurations) {
	test.describe(`VirtualList - ${config.description}`, () => {
		test('should initialize at bottom when initialPosition is bottom', async ({ mount }) => {
			const component = await mount(VirtualListTestWrapper, {
				props: {
					itemCount: config.itemCount,
					batchSize: config.batchSize,
					stickToBottom: true
				}
			});

			const viewport = component.locator('.viewport');
			await expect(viewport).toBeVisible();

			// Wait for initial scroll to complete
			await waitForScrollStability(viewport);

			// Should be at bottom (within 10px tolerance for smooth scrolling)
			const distanceFromBottom = await getDistanceFromBottom(viewport);
			expect(distanceFromBottom).toBeLessThan(10);
		});

		test('should AUTO-SCROLL to bottom when new items added while at bottom', async ({ mount }) => {
			const component = await mount(VirtualListTestWrapper, {
				props: {
					itemCount: config.itemCount,
					batchSize: config.batchSize,
					stickToBottom: true
				}
			});

			const viewport = component.locator('.viewport');
			await waitForScrollStability(viewport);

			// Verify initially at bottom
			let distanceFromBottom = await getDistanceFromBottom(viewport);
			expect(distanceFromBottom).toBeLessThan(10);

			// Record the scrollHeight before adding items
			const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);

			// Click button to add items
			const addButton = component.locator('button', { hasText: 'Add Items' });
			await addButton.click();

			// Wait for items to be added and rendered
			await waitForScrollHeightIncrease(viewport, scrollHeightBefore);

			// Wait for auto-scroll to complete
			await waitForScrollStability(viewport, 2000);

			// Verify scrollHeight actually increased (items were added)
			const { scrollHeight: scrollHeightAfter } = await getScrollProperties(viewport);
			expect(scrollHeightAfter).toBeGreaterThan(scrollHeightBefore);

			// Verify we're STILL at bottom after adding items (this proves auto-scroll happened)
			// Allow 100px tolerance since smooth scrolling and virtual list recalculation may not be pixel-perfect
			distanceFromBottom = await getDistanceFromBottom(viewport);
			expect(distanceFromBottom).toBeLessThan(10);
		});

		test('should NOT auto-scroll when user scrolled up beyond sticky distance (100px)', async ({
			mount,
			page
		}) => {
			const component = await mount(VirtualListTestWrapper, {
				props: {
					itemCount: config.itemCount,
					batchSize: config.batchSize,
					stickToBottom: true
				}
			});

			const viewport = component.locator('.viewport');
			await waitForScrollStability(viewport);

			// Scroll up 300px (beyond STICKY_DISTANCE of 100px)
			const { scrollTop: initialScrollTop } = await getScrollProperties(viewport);
			await scrollTo(viewport, initialScrollTop - 300);

			await page.waitForTimeout(100);

			// Record scroll position and verify we're NOT at bottom
			const { scrollTop: scrollTopAfterScrollUp } = await getScrollProperties(viewport);
			const distanceFromBottom = await getDistanceFromBottom(viewport);
			expect(distanceFromBottom).toBeGreaterThan(200); // Should be around 300px from bottom

			// Record scrollHeight before adding items
			const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);

			// Add new items
			const addButton = component.locator('button', { hasText: 'Add Items' });
			await addButton.click();

			// Wait for new items to render
			await waitForScrollHeightIncrease(viewport, scrollHeightBefore);

			await page.waitForTimeout(300);

			// Verify scrollTop stayed the same (NO auto-scroll happened)
			const { scrollTop: scrollTopAfterNewItems } = await getScrollProperties(viewport);
			expect(scrollTopAfterNewItems).toBe(scrollTopAfterScrollUp);

			// Verify we're still NOT at bottom
			const distanceAfterItems = await getDistanceFromBottom(viewport);
			expect(distanceAfterItems).toBeGreaterThan(100);
		});

		test('should show "New unread" button when scrolled up far and new items arrive', async ({
			mount,
			page
		}) => {
			const component = await mount(VirtualListTestWrapper, {
				props: {
					itemCount: config.itemCount,
					batchSize: config.batchSize,
					stickToBottom: true
				}
			});

			const viewport = component.locator('.viewport');
			await waitForScrollStability(viewport);

			// Scroll to top (far from bottom)
			await scrollTo(viewport, 0);

			await page.waitForTimeout(200);

			// Verify we're far from bottom (>300px triggers the button)
			const distanceFromBottom = await getDistanceFromBottom(viewport);
			expect(distanceFromBottom).toBeGreaterThan(300);

			// Add new items
			const addButton = component.locator('button', { hasText: 'Add Items' });
			await addButton.click();

			// Wait for items to render
			await page.waitForTimeout(500);

			// The "New unread" button should appear (appears when lastDistanceFromBottom > 300)
			const newUnreadButton = page.locator('text="New unread"');
			await expect(newUnreadButton).toBeVisible({ timeout: 2000 });

			// Click the button to scroll to bottom
			await newUnreadButton.click();

			// Wait for scroll animation to complete
			await waitForScrollStability(viewport, 2000);

			// Verify we're now at the bottom
			const finalDistance = await getDistanceFromBottom(viewport);
			expect(finalDistance).toBeLessThan(10);

			// Button should disappear after clicking
			await expect(newUnreadButton).not.toBeVisible();
		});

		test('should maintain bottom position when stickToBottom enabled', async ({ mount }) => {
			// This test verifies the core stick-to-bottom contract:
			// When at bottom and stickToBottom=true, scroll stays at bottom when content changes
			const component = await mount(VirtualListTestWrapper, {
				props: {
					itemCount: config.itemCount,
					batchSize: config.batchSize,
					stickToBottom: true
				}
			});

			const viewport = component.locator('.viewport');
			await waitForScrollStability(viewport);

			// Verify starting at bottom
			let distanceFromBottom = await getDistanceFromBottom(viewport);
			expect(distanceFromBottom).toBeLessThan(10);

			// Add items multiple times and verify we stay at bottom each time
			for (let i = 0; i < 3; i++) {
				const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);

				const addButton = component.locator('button', { hasText: 'Add Items' });
				await addButton.click();

				// Wait for content to grow
				await waitForScrollHeightIncrease(viewport, scrollHeightBefore);

				await waitForScrollStability(viewport, 2000);

				// Should still be at bottom (allow 110px tolerance for virtual list recalculation)
				distanceFromBottom = await getDistanceFromBottom(viewport);
				expect(distanceFromBottom).toBeLessThan(110);
			}
		});
	});
}
