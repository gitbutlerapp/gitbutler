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

	// Toggle footer on (adds 200px element)
	const toggleButton = component.getByTestId("toggle-footer-button");
	await toggleButton.click();

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
