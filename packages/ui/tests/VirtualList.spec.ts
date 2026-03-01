import VirtualListTestWrapper from "./VirtualListTestWrapper.svelte";
import {
	waitForScrollStability,
	waitForScrollHeightIncrease,
	getScrollProperties,
	scrollTo,
	expectAtBottom,
	expectNotAtBottom,
	getVisibleItemIndices,
} from "./test-utils";
import { test, expect } from "@playwright/experimental-ct-svelte";

const config = { itemCount: 20, defaultHeight: 100, asyncContent: { delay: 100, height: 200 } };

/**
 * Click the "Add Item" button and wait for the new item to render
 */
async function addItemAndWait(component: any, viewport: any) {
	const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);
	await component.locator("button", { hasText: "Add Item" }).click();
	await waitForScrollHeightIncrease(viewport, scrollHeightBefore);
}

test("should initialize at bottom when initialPosition is bottom", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});
	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);
});

test("should AUTO-SCROLL to bottom when new items added while at bottom", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});
	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Record the scrollHeight before adding items
	const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);

	// Add item and wait for it to render
	await addItemAndWait(component, viewport);
	await waitForScrollStability(viewport);

	// Verify scrollHeight actually increased (items were added)
	const { scrollHeight: scrollHeightAfter } = await getScrollProperties(viewport);
	expect(scrollHeightAfter).toBeGreaterThan(scrollHeightBefore);

	// Verify we're STILL at bottom after adding items (this proves auto-scroll happened)
	// Use larger tolerance due to rendering under heavy test load
	await expectAtBottom(viewport);
});

test("should NOT auto-scroll when user scrolled up beyond sticky distance", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});
	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	await scrollTo(viewport, 0);
	// Add new item and wait for it to render
	await addItemAndWait(component, viewport);
	// Verify we're still NOT at bottom
	await expectNotAtBottom(viewport);
});

test("should show new unread button when scrolled up and new items added", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Scroll to top (far from bottom)
	await scrollTo(viewport, 0);
	await expectNotAtBottom(viewport);

	// Add new item
	await component.locator("button", { hasText: "Add Item" }).click();

	// The "New unread" button should appear
	const newUnreadButton = component.locator('text="New unread"');
	await expect(newUnreadButton).toBeVisible();

	// Click the button to scroll to bottom
	await newUnreadButton.click();
	await waitForScrollStability(viewport);

	await expectAtBottom(viewport);
	await expect(newUnreadButton).not.toBeVisible();
});

test("should maintain bottom position when stickToBottom enabled", async ({ mount }) => {
	// This test verifies the core stick-to-bottom contract:
	// When at bottom and stickToBottom=true, scroll stays at bottom when content changes
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	await expectAtBottom(viewport);

	// Add items multiple times and verify we stay at bottom each time
	for (let i = 0; i < 3; i++) {
		await addItemAndWait(component, viewport);
		await waitForScrollStability(viewport);

		// Use larger tolerance due to async content potentially loading
		await expectAtBottom(viewport);
	}
});

test("should initialize at a specific startIndex", async ({ mount }) => {
	const startIndex = 10;
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			startIndex,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Verify the specified item is visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(startIndex);

	// Verify we're not at top or bottom
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBeGreaterThan(0);
	await expectNotAtBottom(viewport);
});

test("should jump to a specific index using jumpToIndex method", async ({ mount }) => {
	const targetIndex = 15;
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Initially, we should be at the top
	const { scrollTop: initialScrollTop } = await getScrollProperties(viewport);
	expect(initialScrollTop).toBe(0);

	// Set the jump-to index and click the button
	const input = component.getByTestId("jump-to-index-input");
	await input.fill(targetIndex.toString());
	const jumpButton = component.getByTestId("jump-to-index-button");
	await jumpButton.click();
	await waitForScrollStability(viewport);

	// Verify the target item is now visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(targetIndex);

	// Verify we scrolled down from the top
	const { scrollTop: finalScrollTop } = await getScrollProperties(viewport);
	expect(finalScrollTop).toBeGreaterThan(0);
});

test("should initialize at top by default", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: false,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Should be at the top
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBe(0);

	// First items should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(0);
	expect(visibleIndices).toContain(1);

	// Should NOT be at bottom
	await expectNotAtBottom(viewport);
});

test("should call onloadmore callback when scrolled to bottom", async ({ mount }) => {
	let loadMoreCalled = false;

	async function onloadmore() {
		loadMoreCalled = true;
	}

	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined, // Remove async content for simpler test
			onloadmore,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Scroll to bottom by clicking the button
	const scrollButton = component.getByTestId("scroll-to-bottom-button");
	await scrollButton.click();

	await waitForScrollStability(viewport);

	// Wait for debounce (50ms) + buffer
	await component.page().waitForTimeout(200);

	// Verify the callback was called
	expect(loadMoreCalled).toBe(true);
});

test("should initialize at bottom with very tall items when stickToBottom enabled", async ({
	mount,
}) => {
	// This tests the scenario where items are much taller than the viewport
	// The list should still initialize at the bottom correctly
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 10,
			defaultHeight: 500, // Very tall items (viewport is only 400px)
			stickToBottom: true,
			asyncContent: undefined, // No async content to keep test simple
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Should be scrolled to the bottom
	await expectAtBottom(viewport);

	// The last item should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(9); // Last item (0-indexed)

	// Verify scroll position is not at top
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBeGreaterThan(0);
});

test("should stick to bottom when last item expands even with stickToBottom disabled", async ({
	mount,
}) => {
	// When manually scrolled to bottom (without stickToBottom), expanding the last item
	// should still keep us at the bottom - this is important for expanding collapsed diffs
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: false,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Initially at top
	const { scrollTop: initialScrollTop } = await getScrollProperties(viewport);
	expect(initialScrollTop).toBe(0);

	// Scroll to bottom manually
	const scrollButton = component.getByTestId("scroll-to-bottom-button");
	await scrollButton.click();
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Record scroll height before expansion
	const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);

	// Expand the last item
	await component.locator("button", { hasText: "Expand Last" }).click();
	await waitForScrollStability(viewport);

	// Verify the item actually expanded
	const { scrollHeight: scrollHeightAfter } = await getScrollProperties(viewport);
	expect(scrollHeightAfter).toBeGreaterThan(scrollHeightBefore);

	// Should still be at bottom after expansion
	await expectAtBottom(viewport);
});

test("should stick to bottom when footer toggled in children snippet", async ({ mount }) => {
	// This tests that stickToBottom works correctly when content is added to the children snippet
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Initially at bottom
	await expectAtBottom(viewport);

	// Record scrollHeight before toggling footer
	const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);

	// Toggle footer on (adds 200px element) in the children snippet
	await component.getByTestId("toggle-footer-button").click();

	// Wait for footer to appear
	const footer = component.getByTestId("footer");
	await expect(footer).toBeVisible();

	// Wait for scroll to stabilize
	await waitForScrollStability(viewport);

	// Verify scrollHeight increased (footer was added)
	const { scrollHeight: scrollHeightAfter } = await getScrollProperties(viewport);
	expect(scrollHeightAfter).toBeGreaterThan(scrollHeightBefore);

	// Should STILL be at bottom (stickToBottom should have auto-scrolled)
	await expectAtBottom(viewport);
});

test("should stick to bottom when footer toggled and item appended simultaneously", async ({
	mount,
}) => {
	// Reproduces a race between the items $effect (tail append) and the children
	// resizeObserver (footer appears) when both state changes are batched into a
	// single Svelte update. The footer is 200px > NEAR_BOTTOM_THRESHOLD.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Initially at bottom
	await expectAtBottom(viewport);

	// Record scrollHeight before the combined action
	const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);

	// Toggle footer on AND append an item in a single batched state update
	await component.getByTestId("toggle-footer-and-add-item-button").click();

	// Wait for footer to appear
	const footer = component.getByTestId("footer");
	await expect(footer).toBeVisible();

	// Wait for scroll to stabilize
	await waitForScrollStability(viewport);

	// Verify scrollHeight increased (both footer and new item were added)
	const { scrollHeight: scrollHeightAfter } = await getScrollProperties(viewport);
	expect(scrollHeightAfter).toBeGreaterThan(scrollHeightBefore);

	// Should STILL be at bottom (stickToBottom should have auto-scrolled)
	await expectAtBottom(viewport);
});

test("should maintain scroll position when items are prepended with stickToBottom", async ({
	mount,
}) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Prepend an item (adds to the beginning of the list)
	const { scrollHeight: scrollHeightBefore } = await getScrollProperties(viewport);
	await component.getByTestId("prepend-item-button").click();
	await waitForScrollHeightIncrease(viewport, scrollHeightBefore);
	await waitForScrollStability(viewport);

	// With stickToBottom + headChanged, scroll should compensate to keep position
	// We should NOT jump to bottom since only the head changed
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBeGreaterThan(0);
});

test("should call onloadmore near the top when stickToBottom is enabled", async ({ mount }) => {
	let loadMoreCalled = false;

	async function onloadmore() {
		loadMoreCalled = true;
	}

	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			asyncContent: undefined,
			onloadmore,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// First scroll to an intermediate position to clear any pending skipNextScrollEvent
	// flags that may have been set during stickToBottom initialization.
	const { scrollHeight } = await getScrollProperties(viewport);
	await scrollTo(viewport, Math.floor(scrollHeight / 2));
	await waitForScrollStability(viewport);

	// Now scroll to the top — in stickToBottom mode, this is where onloadmore should fire
	await scrollTo(viewport, 0);
	await waitForScrollStability(viewport);

	// Wait for debounce (50ms) + buffer
	await component.page().waitForTimeout(200);

	expect(loadMoreCalled).toBe(true);
});

test("should fire onVisibleChange callback with visible range", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// The visible range should have been reported via onVisibleChange
	const startText = await component.getByTestId("visible-start").textContent();
	const endText = await component.getByTestId("visible-end").textContent();
	const visibleStart = parseInt(startText || "-1", 10);
	const visibleEnd = parseInt(endText || "-1", 10);

	// visibleStart should be 0 (initialized at top)
	expect(visibleStart).toBe(0);
	// visibleEnd should be a reasonable number based on viewport/item height
	expect(visibleEnd).toBeGreaterThan(0);
	expect(visibleEnd).toBeLessThanOrEqual(20);
});

test("should show scroll-to-bottom button when showBottomButton is enabled and scrolled up", async ({
	mount,
}) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			showBottomButton: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Button should NOT be visible when at bottom
	const scrollButtonContainer = component.locator(".feed-actions__scroll-to-bottom");
	await expect(scrollButtonContainer).not.toBeVisible();

	// First scroll to an intermediate position to clear any pending skipNextScrollEvent
	// flags set during stickToBottom initialization.
	const { scrollHeight } = await getScrollProperties(viewport);
	await scrollTo(viewport, Math.floor(scrollHeight / 2));
	await waitForScrollStability(viewport);

	// Scroll far up (beyond SCROLL_DOWN_THRESHOLD of 600px)
	await scrollTo(viewport, 0);
	await waitForScrollStability(viewport);

	// Button should now be visible
	await expect(scrollButtonContainer).toBeVisible();

	// Click the button — should scroll to bottom
	await scrollButtonContainer.locator("button").click();
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);
});

test("should handle empty items list without errors", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 0,
			defaultHeight: 100,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");

	// Should render without errors
	await expect(viewport).toBeVisible();

	// No items should be rendered
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toHaveLength(0);

	// scrollTop should be 0
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBe(0);
});

test("should handle a single item shorter than viewport", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 1,
			defaultHeight: 100,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// The single item should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(0);
	expect(visibleIndices).toHaveLength(1);

	// Should be at top (no scrolling needed)
	const { scrollTop, scrollHeight, clientHeight } = await getScrollProperties(viewport);
	expect(scrollTop).toBe(0);
	// scrollHeight should be <= clientHeight (content fits in viewport)
	expect(scrollHeight).toBeLessThanOrEqual(clientHeight + 1);
});

test("should handle items being replaced entirely", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Initially should have original items
	const initialIndices = await getVisibleItemIndices(viewport);
	expect(initialIndices.length).toBeGreaterThan(0);

	// Replace all items
	await component.getByTestId("replace-items-button").click();
	await waitForScrollStability(viewport);

	// Should still render items without errors
	const newIndices = await getVisibleItemIndices(viewport);
	expect(newIndices.length).toBeGreaterThan(0);

	// Verify the new content is rendered
	const firstItem = viewport.locator(".test-item").first();
	await expect(firstItem).toContainText("Replaced");
});

test("should not crash when items are removed", async ({ mount }) => {
	// The renderRange.end can exceed items.length after removal.
	// heightMap.length is set to items.length (line 545), but renderRange
	// might still reference indices beyond the new length.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	const indicesBefore = await getVisibleItemIndices(viewport);
	expect(indicesBefore.length).toBeGreaterThan(0);

	// Remove several items
	for (let i = 0; i < 5; i++) {
		await component.getByTestId("remove-last-button").click();
	}
	await waitForScrollStability(viewport);

	// Should still render items without errors
	const indicesAfter = await getVisibleItemIndices(viewport);
	expect(indicesAfter.length).toBeGreaterThan(0);

	// All rendered indices should be valid (within the new item count)
	for (const idx of indicesAfter) {
		expect(idx).toBeLessThan(15); // 20 - 5 = 15
		expect(idx).toBeGreaterThanOrEqual(0);
	}
});

test("should show 'New unread' when batch items added while scrolled up with stickToBottom", async ({
	mount,
	page,
}) => {
	// When stickToBottom is on but user has scrolled up, adding many items at once
	// should show the "New unread" indicator, not force-scroll to bottom.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	const scrollButton = component.getByTestId("scroll-to-top-button");
	await scrollButton.click();

	await waitForScrollStability(viewport);
	await expectNotAtBottom(viewport);

	// Add 10 items at once
	await page.waitForTimeout(1000);
	await component.getByTestId("add-batch-button").click();
	await waitForScrollStability(viewport);

	// Should NOT have auto-scrolled to bottom
	await expectNotAtBottom(viewport);

	// "New unread" button should be visible
	const newUnreadButton = component.locator('text="New unread"');
	await expect(newUnreadButton).toBeVisible();
});

test("should jump to index 0 from a scrolled position", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Scroll to bottom first
	const scrollButton = component.getByTestId("scroll-to-bottom-button");
	await scrollButton.click();
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Jump to index 0
	const input = component.getByTestId("jump-to-index-input");
	await input.fill("0");
	await component.getByTestId("jump-to-index-button").click();
	await waitForScrollStability(viewport);

	// Should be back at the top
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBe(0);

	// First item should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(0);
});

test("should jump to the last item", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Initially at top
	const { scrollTop: initialScrollTop } = await getScrollProperties(viewport);
	expect(initialScrollTop).toBe(0);

	// Jump to last item (index 19 for 20 items)
	const input = component.getByTestId("jump-to-index-input");
	await input.fill("19");
	await component.getByTestId("jump-to-index-button").click();
	await waitForScrollStability(viewport);

	// Last item should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(19);

	// Should have scrolled down
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBeGreaterThan(0);
});

test("should update onVisibleChange when scrolling", async ({ mount }) => {
	// onVisibleChange should fire not just at init, but as the user scrolls.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Read initial visible range
	const initialStart = parseInt(
		(await component.getByTestId("visible-start").textContent()) || "-1",
		10,
	);
	expect(initialStart).toBe(0);

	// Scroll to bottom
	const scrollButton = component.getByTestId("scroll-to-bottom-button");
	await scrollButton.click();
	await waitForScrollStability(viewport);

	// Visible range should have changed — start should no longer be 0
	const scrolledStart = parseInt(
		(await component.getByTestId("visible-start").textContent()) || "-1",
		10,
	);
	expect(scrolledStart).toBeGreaterThan(0);
});

test("should trigger onloadmore when content is shorter than viewport", async ({ mount }) => {
	// Line 268: if (viewport.scrollHeight <= viewport.clientHeight) return true;
	// For a short list, onloadmore should fire immediately without scrolling.
	let loadMoreCalled = false;

	async function onloadmore() {
		loadMoreCalled = true;
	}

	await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 2,
			defaultHeight: 100,
			asyncContent: undefined,
			onloadmore,
		},
	});

	// Wait for initialization + debounce (50ms) + buffer
	await new Promise((resolve) => setTimeout(resolve, 500));

	expect(loadMoreCalled).toBe(true);
});

test("should only render items near the viewport (virtualization contract)", async ({ mount }) => {
	// The core invariant of virtual scrolling: items far from the viewport
	// should NOT be in the DOM at all.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 100,
			defaultHeight: 100,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// At the top, we should NOT have all 100 items rendered
	const renderedIndices = await getVisibleItemIndices(viewport);
	expect(renderedIndices.length).toBeLessThan(100);
	// Should have at most ~viewport height / item height + buffer items
	// Viewport is 400px, items are 100px, so ~4-8 items max
	expect(renderedIndices.length).toBeLessThanOrEqual(10);

	// Indices near the top should be present
	expect(renderedIndices).toContain(0);

	// Indices far from the viewport should NOT be present
	expect(renderedIndices).not.toContain(50);
	expect(renderedIndices).not.toContain(99);
});

test("should keep bottom position after multiple rapid appends with stickToBottom", async ({
	mount,
}) => {
	// Rapid appends can race with the async items $effect (line 544).
	// The previousItems snapshot (line 582) is taken synchronously, but the
	// async handler might still be running from the previous update.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Rapidly add 5 items without waiting between clicks
	const addButton = component.locator("button", { hasText: "Add Item" });
	for (let i = 0; i < 5; i++) {
		await addButton.click();
	}

	// Wait for everything to settle
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);

	// Should still be at bottom after rapid appends
	await expectAtBottom(viewport);
});

test("should handle prepending items without stickToBottom", async ({ mount }) => {
	// Without stickToBottom, the headChanged scroll compensation (line 557-559)
	// does NOT run. Prepending items should still work without crashing,
	// and the scroll position might shift.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: false,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Initially at top
	const { scrollTop: initialScrollTop } = await getScrollProperties(viewport);
	expect(initialScrollTop).toBe(0);

	// Prepend an item
	await component.getByTestId("prepend-item-button").click();
	await waitForScrollStability(viewport);

	// Should not crash; items should still render
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices.length).toBeGreaterThan(0);

	// The prepended item (index 0) should be visible since we were at the top
	expect(visibleIndices).toContain(0);
});

test("should handle removing items until list is shorter than viewport", async ({ mount }) => {
	// Start with items that overflow, remove until they fit in the viewport.
	// This tests the transition from scrollable to non-scrollable state.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 6,
			defaultHeight: 100,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Remove items until we have fewer than can fill viewport (400px / 100px = 4)
	for (let i = 0; i < 4; i++) {
		await component.getByTestId("remove-last-button").click();
	}
	await waitForScrollStability(viewport);

	// Should have 2 items remaining — fits in viewport
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toHaveLength(2);

	// scrollTop should be 0 (no overflow)
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBe(0);
});

test("should handle removing all items", async ({ mount }) => {
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 3,
			defaultHeight: 100,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Remove all items
	for (let i = 0; i < 3; i++) {
		await component.getByTestId("remove-last-button").click();
	}
	await waitForScrollStability(viewport);

	// Should render without errors, no items visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toHaveLength(0);
});

// ---------------------------------------------------------------------------
// Bug-hunting tests: targeting specific fragile code paths
// ---------------------------------------------------------------------------

test("should not snap back to jumpToIndex position when items are added afterwards", async ({
	mount,
}) => {
	// BUG TARGET: `lastJumpToIndex` (line 157) is set in jumpToIndex() but
	// NEVER cleared. In itemObserver path 2 (line 196-204), if
	// `lastJumpToIndex !== undefined && lastScrollDirection === undefined`,
	// it force-sets scrollTop back to the jump position on every item resize.
	// After jumping and then adding items, new items resizing could snap
	// the scroll position back to the old jump target.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Jump to index 10 (middle of list)
	const input = component.getByTestId("jump-to-index-input");
	await input.fill("10");
	await component.getByTestId("jump-to-index-button").click();
	await waitForScrollStability(viewport);

	const visibleAfterJump = await getVisibleItemIndices(viewport);
	expect(visibleAfterJump).toContain(10);

	// Now scroll to bottom (this sets lastScrollDirection = "down")
	const scrollButton = component.getByTestId("scroll-to-bottom-button");
	await scrollButton.click();
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Add new items — these will trigger itemObserver as they render.
	// If lastJumpToIndex is still 10 and lastScrollDirection becomes undefined,
	// path 2 would snap back to index 10 instead of staying at bottom.
	for (let i = 0; i < 3; i++) {
		await component.locator("button", { hasText: "Add Item" }).click();
	}
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(200);

	// Should still be at/near bottom, NOT snapped back to index 10
	await expectAtBottom(viewport);
});

test("should stay at bottom when items are replaced with stickToBottom", async ({ mount }) => {
	// BUG TARGET: When items are replaced entirely, BOTH headChanged and
	// tailChanged are true. The items $effect (lines 557-568) checks:
	//   if (headChanged && !tailChanged) { ... }
	//   else if (tailChanged && !headChanged) { ... }
	// When BOTH are true, NEITHER branch executes, so no auto-scroll
	// to bottom happens. With stickToBottom, we'd expect to stay at bottom.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Replace all items (changes both head and tail)
	await component.getByTestId("replace-items-button").click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);

	// With stickToBottom, we expect to end up at bottom after replacement.
	// This may FAIL if the both-changed path doesn't handle auto-scroll.
	await expectAtBottom(viewport);
});

test("should initialize correctly when starting from empty and adding items", async ({ mount }) => {
	// Tests the transition: empty list → items added.
	// The items $effect (line 534) returns early for empty items.
	// When items are pushed, !isInitialized() triggers initializeAt().
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 0,
			defaultHeight: 100,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");

	// Start empty
	let visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toHaveLength(0);

	// Add items
	for (let i = 0; i < 5; i++) {
		await component.locator("button", { hasText: "Add Item" }).click();
	}
	await waitForScrollStability(viewport);

	// Items should now be rendered
	visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices.length).toBeGreaterThan(0);
	expect(visibleIndices).toContain(0);
});

test("should initialize at bottom when starting from empty with stickToBottom", async ({
	mount,
}) => {
	// Same as above but with stickToBottom: the initializeAt call (line 547)
	// should use items.length - 1 to start at the end.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 0,
			defaultHeight: 100,
			stickToBottom: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");

	// Add enough items to overflow the viewport
	for (let i = 0; i < 10; i++) {
		await component.locator("button", { hasText: "Add Item" }).click();
	}
	await waitForScrollStability(viewport);

	// Should be at bottom with stickToBottom
	await expectAtBottom(viewport);

	// Last item should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(9);
});

test("should maintain bottom when items removed with stickToBottom", async ({ mount }) => {
	// When at bottom with stickToBottom and items are removed (countDelta <= 0),
	// the else branch (line 571) checks stickToBottom && previousDistance < STICKY_DISTANCE,
	// then recalculates and scrolls to bottom if needed.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Remove several items while at bottom
	for (let i = 0; i < 5; i++) {
		await component.getByTestId("remove-last-button").click();
	}
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(200);

	// Should still be at bottom
	await expectAtBottom(viewport);
});

test("should remove all items then re-add and re-initialize", async ({ mount }) => {
	// Tests the full lifecycle: initialized → empty → re-initialized.
	// After removing all items, renderRange.end becomes stale.
	// Adding items back triggers !isInitialized() check which should
	// re-initialize, but isInitialized() checks renderRange.end !== 0,
	// which may still be non-zero from the old state.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 3,
			defaultHeight: 100,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Verify initial state
	let visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices.length).toBeGreaterThan(0);

	// Remove all items
	for (let i = 0; i < 3; i++) {
		await component.getByTestId("remove-last-button").click();
	}
	await waitForScrollStability(viewport);

	visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toHaveLength(0);

	// Re-add items
	for (let i = 0; i < 5; i++) {
		await component.locator("button", { hasText: "Add Item" }).click();
	}
	await waitForScrollStability(viewport);

	// Should have re-initialized and be showing items
	visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices.length).toBeGreaterThan(0);
	expect(visibleIndices).toContain(0);
});

test("should add items without stickToBottom while scrolled to middle", async ({ mount }) => {
	// Without stickToBottom, the countDelta > 0 path (line 553) enters
	// the `if (stickToBottom)` block — which is false. So only
	// updateOffsets() runs. The new items should appear at the end
	// without disrupting the current scroll position.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: false,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Scroll to middle
	const { scrollHeight } = await getScrollProperties(viewport);
	const middlePosition = Math.floor(scrollHeight / 2);
	await scrollTo(viewport, middlePosition);
	await waitForScrollStability(viewport);

	const { scrollTop: scrollBefore } = await getScrollProperties(viewport);

	// Add items at the end
	for (let i = 0; i < 3; i++) {
		await component.locator("button", { hasText: "Add Item" }).click();
	}
	await waitForScrollStability(viewport);

	// Scroll position should be approximately preserved (not jumped to top or bottom)
	const { scrollTop: scrollAfter } = await getScrollProperties(viewport);
	expect(Math.abs(scrollAfter - scrollBefore)).toBeLessThan(50);
});

test("should handle jumpToIndex then scroll then add items without position corruption", async ({
	mount,
}) => {
	// BUG TARGET: After jumpToIndex, lastJumpToIndex persists (line 496).
	// Path 2 in itemObserver (line 196-204) fires when
	// `(lastJumpToIndex !== undefined) && lastScrollDirection === undefined`.
	// After jumping then scrolling, lastScrollDirection is "up"/"down",
	// so path 2 shouldn't fire. But if lastScrollTop is 0 (line 611),
	// `0 && ...` is falsy, so direction becomes undefined, re-enabling path 2.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Jump to index 10
	const input = component.getByTestId("jump-to-index-input");
	await input.fill("10");
	await component.getByTestId("jump-to-index-button").click();
	await waitForScrollStability(viewport);

	// Now scroll to TOP (scrollTop = 0). This causes lastScrollTop to be 0.
	await scrollTo(viewport, 0);
	await waitForScrollStability(viewport);
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBe(0);

	// Add items — this causes item resizes. With lastScrollTop=0,
	// scroll direction detection fails (0 && ... is falsy).
	// If lastScrollDirection becomes undefined and lastJumpToIndex is still 10,
	// path 2 would snap scrollTop to index 10's position.
	await component.locator("button", { hasText: "Add Item" }).click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(200);

	// We should still be near the top, NOT snapped back to index 10
	const { scrollTop: finalScrollTop } = await getScrollProperties(viewport);
	expect(finalScrollTop).toBeLessThan(200);
});

test("should not snap back to startIndex when scrolling away and items resize", async ({
	mount,
}) => {
	// BUG TARGET: itemObserver path 2 (line 196-197) checks:
	//   (lastJumpToIndex !== undefined || startIndex) && lastScrollDirection === undefined
	// If startIndex is provided (truthy), this is always true when
	// lastScrollDirection === undefined. After scrolling, direction is set,
	// but at scrollTop=0, direction detection fails (lastScrollTop=0 is falsy).
	// Items with async content resize after a delay, potentially triggering
	// path 2 and snapping back to startIndex.
	const startIndex = 10;
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 30,
			defaultHeight: 100,
			startIndex,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Verify we started at index 10
	let visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(startIndex);

	// Scroll to bottom
	const scrollButton = component.getByTestId("scroll-to-bottom-button");
	await scrollButton.click();
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// The scroll position should stay at bottom, not snap back to startIndex
	await component.page().waitForTimeout(300);
	await expectAtBottom(viewport);

	// Scroll to top
	await scrollTo(viewport, 0);
	await waitForScrollStability(viewport);

	// Should be at top, not snapped to startIndex
	const { scrollTop } = await getScrollProperties(viewport);
	expect(scrollTop).toBe(0);
	visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(0);
});

// ---------------------------------------------------------------------------
// IRC-style regression tests: defaultHeight mismatch + stickToBottom + loadMore
// Reproduces the bug where IrcMessages with ~200 messages doesn't stick to
// bottom on initialization. The IRC component uses defaultHeight={16} with
// renderDistance={100} and stickToBottom, but actual messages are much taller.
// ---------------------------------------------------------------------------

const ircConfig = {
	defaultHeight: 16,
	stickToBottom: true,
	renderDistance: 100,
	asyncContent: undefined,
};

test("should stick to bottom with 200 items and small defaultHeight", async ({ mount }) => {
	// Core scenario: 200 items with defaultHeight=16 but actual height ~100px.
	// The massive height mismatch (16 vs 100) means initializeAt calculates
	// scroll positions using wrong estimates for unmeasured items.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 200,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Must be at the bottom after initialization
	await expectAtBottom(viewport);

	// Last items should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(199);
});

test("should stick to bottom when loadMore prepends items during init", async ({ mount }) => {
	// The critical bug: with stickToBottom + small defaultHeight, during
	// initializeAt the scroll position is temporarily 0. shouldTriggerLoadMore
	// checks getDistanceFromTop() < 300 for stickToBottom mode, which is true
	// when scrollTop=0. This triggers onloadmore which prepends items,
	// disrupting the initialization scroll position.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 200,
			loadMorePrependCount: 50,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	// Give extra time for loadMore + prepend + re-render to settle
	await component.page().waitForTimeout(500);
	await waitForScrollStability(viewport);

	// Must be at the bottom despite loadMore firing during init
	await expectAtBottom(viewport);
});

test("should not fire loadMore excessively during stickToBottom init", async ({ mount }) => {
	// If loadMore fires during initialization, it means the component
	// is incorrectly detecting a "near top" scroll position before
	// the scroll position has been properly set by initializeAt.
	let loadMoreCallCount = 0;

	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 200,
			onloadmore: async () => {
				loadMoreCallCount++;
			},
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);

	// loadMore should not have fired during initialization — we just opened
	// the view with stickToBottom and haven't scrolled near the top yet.
	// At most 1 is acceptable if it fires right after init completes.
	expect(loadMoreCallCount).toBeLessThanOrEqual(1);
});

test("should stick to bottom with async content and loadMore prepending", async ({ mount }) => {
	// Closest reproduction of the real IrcMessages scenario:
	// - Many items with defaultHeight far smaller than actual rendered height
	// - stickToBottom + renderDistance=100
	// - asyncContent that causes items to grow after initial render
	// - loadMore that prepends items (simulating history loading)
	// The async resize events fire during/after init, each potentially
	// triggering shouldTriggerLoadMore with a wrong scroll position.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 200,
			defaultHeight: 16,
			stickToBottom: true,
			renderDistance: 100,
			asyncContent: { delay: 50, height: 200 },
			loadMorePrependCount: 50,
		},
	});

	const viewport = component.locator(".viewport");
	// Give generous time for async content to load and resize observers to fire
	await component.page().waitForTimeout(1000);
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(500);
	await waitForScrollStability(viewport);

	await expectAtBottom(viewport);
});

test("should not spiral loadMore with async content resizing", async ({ mount }) => {
	// Tests the specific failure mode: async content causes item resizes
	// which trigger itemObserver → recalculateRanges → shouldTriggerLoadMore.
	// With stickToBottom and inaccurate defaultHeight, the estimated scroll
	// position can transiently appear "near top", falsely triggering loadMore.
	let loadMoreCallCount = 0;

	const component = await mount(VirtualListTestWrapper, {
		props: {
			itemCount: 200,
			defaultHeight: 16,
			stickToBottom: true,
			renderDistance: 100,
			asyncContent: { delay: 50, height: 200 },
			onloadmore: async () => {
				loadMoreCallCount++;
			},
		},
	});

	const viewport = component.locator(".viewport");
	await component.page().waitForTimeout(1000);
	await waitForScrollStability(viewport);

	// With async content, loadMore might fire once or twice during the
	// resize storm, but it should not spiral into many calls.
	expect(loadMoreCallCount).toBeLessThanOrEqual(2);

	// And we must still end up at the bottom
	await expectAtBottom(viewport);
});

test("should stick to bottom with variable-height items", async ({ mount }) => {
	// Real IRC messages have wildly different heights (16px short text to 300px+
	// code blocks). With defaultHeight=16, the estimate is close for short
	// messages but 10-20x wrong for long ones. This creates asymmetric errors
	// in the height calculation that don't cancel out as cleanly as uniform items.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 200,
			variableHeights: true,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Must be at the bottom
	await expectAtBottom(viewport);

	// Last items should be visible
	const visibleIndices = await getVisibleItemIndices(viewport);
	expect(visibleIndices).toContain(199);
});

// ---------------------------------------------------------------------------
// Bug fix regression tests: HEAD prepend shifting, observer cascade, chat switching
// These tests cover the specific bugs found via production debug logging:
// 1. HEAD prepend must shift heightMap + renderRange (not just scrollBy)
// 2. Observer path3 subpixel oscillation must not cascade
// 3. Switching views (both head+tail changed) must re-initialize cleanly
// ---------------------------------------------------------------------------

test("should preserve bottom position when prepending large batch", async ({ mount }) => {
	// Regression for the bug where scrollBy(countDelta * defaultHeight) used wrong
	// estimates. With defaultHeight=16 and actual heights ~100px, the 50*16=800px
	// compensation was far too small. The fix shifts renderRange and heightMap so
	// the compensation exactly matches the offset.top change.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 100,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Record the last visible item before prepend
	const indicesBefore = await getVisibleItemIndices(viewport);
	const lastVisibleBefore = Math.max(...indicesBefore);

	// Prepend 50 items at the front
	await component.getByTestId("prepend-batch-button").click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);
	await waitForScrollStability(viewport);

	// The same items should still be visible (now at shifted indices).
	// Item that was at index N is now at index N+50.
	const indicesAfter = await getVisibleItemIndices(viewport);
	const lastVisibleAfter = Math.max(...indicesAfter);
	expect(lastVisibleAfter).toBeGreaterThanOrEqual(lastVisibleBefore + 50 - 5);

	// Must still be near the bottom — the whole point of the fix.
	await expectAtBottom(viewport);
});

test("should stay at bottom after multiple sequential prepends", async ({ mount }) => {
	// Tests that the heightMap shifting works correctly across multiple prepends.
	// Each prepend must shift the existing cached heights further right.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 50,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Prepend 3 batches of 50 items each (50 → 100 → 150 → 200)
	for (let i = 0; i < 3; i++) {
		await component.getByTestId("prepend-batch-button").click();
		await waitForScrollStability(viewport);
		await component.page().waitForTimeout(200);
	}
	await waitForScrollStability(viewport);

	// After all prepends, the original items are at indices 150-199.
	// We must still be at the bottom seeing those items.
	await expectAtBottom(viewport);
	const indices = await getVisibleItemIndices(viewport);
	expect(indices).toContain(199);
});

test("should re-initialize at bottom when items are fully replaced with stickToBottom", async ({
	mount,
}) => {
	// Regression for the bug where switching IRC channels left the list in the
	// middle. When items are completely replaced (both head and tail change),
	// the old heightMap and renderRange are stale. The fix explicitly resets
	// state and triggers a fresh initializeAt.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 100,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Switch to "Chat 1" (74 items, completely different IDs)
	await component.getByTestId("switch-chat-button").click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);
	await waitForScrollStability(viewport);

	// Must be at bottom after switching
	await expectAtBottom(viewport);

	// Verify the new chat's items are rendered
	const firstItem = viewport.locator(".test-item").first();
	await expect(firstItem).toContainText("Chat1");
});

test("should end at bottom after rapid item replacement with stickToBottom", async ({ mount }) => {
	// Multiple rapid switches stress-test the re-initialization path.
	// Each switch clears stale state and re-initializes from scratch.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 80,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Switch chats 4 times rapidly
	for (let i = 0; i < 4; i++) {
		await component.getByTestId("switch-chat-button").click();
		await component.page().waitForTimeout(50);
	}

	// Wait for everything to settle
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);
	await waitForScrollStability(viewport);

	// Must be at bottom
	await expectAtBottom(viewport);

	// Verify the latest chat's items are rendered
	const firstItem = viewport.locator(".test-item").first();
	await expect(firstItem).toContainText("Chat4");
});

test("should end at bottom after item replacement then prepend", async ({ mount }) => {
	// The full IRC sequence: switch chat → initialize → loadMore fires →
	// history prepended. Tests the interaction between the re-initialization
	// fix and the HEAD prepend fix.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 100,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Switch chat (triggers re-initialization)
	await component.getByTestId("switch-chat-button").click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(200);

	// Now prepend history (HEAD prepend on the new chat)
	await component.getByTestId("prepend-batch-button").click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);
	await waitForScrollStability(viewport);

	// Must still be at bottom after both operations
	await expectAtBottom(viewport);
});

test("should not cascade observer when already at bottom", async ({ mount }) => {
	// Regression for the observer path3 subpixel oscillation bug.
	// After initializeAt completes at the bottom (distFromBottom ≈ 0.27px),
	// observer path3 used to call scrollToBottom repeatedly. The scrollHeight
	// would oscillate by 1px (e.g., 1354/1355), causing scrollTop to bounce,
	// which set lastScrollDirection="up" and triggered a cascade of range
	// expansions that drifted the scroll position away from the bottom.
	// The fix adds a >2px guard to path3.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 200,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Wait for any potential cascade to occur — the old bug caused the
	// scroll position to drift away from bottom over ~500ms.
	await component.page().waitForTimeout(500);

	// Must still be at the bottom, not drifted away by oscillation
	await expectAtBottom(viewport);
});

test("should not produce degenerate range when replacing items", async ({ mount }) => {
	// Regression for recalculateRanges producing start=0, end=0 when items
	// change dramatically. The degenerate range guard prevents this, and the
	// explicit headChanged && tailChanged handler in the items effect ensures
	// clean re-initialization instead of relying on recalculate.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...config,
			stickToBottom: true,
			asyncContent: undefined,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);
	await expectAtBottom(viewport);

	// Replace with entirely different items
	await component.getByTestId("replace-items-button").click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);

	// The list should be properly initialized with the new items
	const indices = await getVisibleItemIndices(viewport);
	expect(indices.length).toBeGreaterThan(0);

	// With stickToBottom, we should end up at the bottom
	await expectAtBottom(viewport);

	// Verify the new content is actually rendered
	const firstItem = viewport.locator(".test-item").first();
	await expect(firstItem).toContainText("Replaced");
});

test("should handle prepend at scrollTop=0 correctly", async ({ mount }) => {
	// Edge case: if the user is already at the top when a HEAD prepend occurs
	// (e.g., they scrolled up manually), the scrollBy compensation should
	// shift the scroll position so the original items stay in view.
	// Uses itemHeight=16 matching defaultHeight=16 for accurate compensation.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 100,
			itemHeight: 16,
		},
	});

	const viewport = component.locator(".viewport");
	await waitForScrollStability(viewport);

	// Scroll to the top
	await scrollTo(viewport, 0);
	await waitForScrollStability(viewport);

	const { scrollTop: scrollTopBefore } = await getScrollProperties(viewport);
	expect(scrollTopBefore).toBe(0);

	// Prepend 50 items
	await component.getByTestId("prepend-batch-button").click();
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);

	// scrollTop should have increased to compensate for the prepended items
	const { scrollTop: scrollTopAfter } = await getScrollProperties(viewport);
	expect(scrollTopAfter).toBeGreaterThan(0);

	// The original first items (now at index 50+) should still be near the viewport
	const indices = await getVisibleItemIndices(viewport);
	const hasOriginalItems = indices.some((i) => i >= 50);
	expect(hasOriginalItems).toBe(true);
});

test("should not trigger items-replaced reset on initial load", async ({ mount }) => {
	// Regression: when previousItems is empty (length 0), both headChanged and
	// tailChanged default to true (ternary fallback). Without the
	// previousItems.length > 0 guard, this incorrectly enters the "items replaced"
	// branch, resetting heightMap and renderRange. The result is a broken
	// initializeAt that only measures a few items, leaving distFromBottom far off.
	// This test starts with 0 items and adds a batch, simulating the initial load
	// pattern seen in CodegenMessages.svelte.
	const component = await mount(VirtualListTestWrapper, {
		props: {
			...ircConfig,
			itemCount: 0,
			itemHeight: 16,
		},
	});

	const viewport = component.locator(".viewport");

	// Start with 0 items, then add a batch (simulating initial data load)
	await component.getByTestId("add-batch-button").click(); // adds 10 items
	await component.getByTestId("add-batch-button").click(); // 20 items
	await component.getByTestId("add-batch-button").click(); // 30 items
	await waitForScrollStability(viewport);
	await component.page().waitForTimeout(300);

	// With stickToBottom + initial load, we must end up at the bottom
	await expectAtBottom(viewport);

	// Verify a reasonable number of items are rendered (not just 2 out of 30)
	const indices = await getVisibleItemIndices(viewport);
	expect(indices.length).toBeGreaterThan(5);
});
