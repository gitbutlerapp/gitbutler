import HardWrapPluginTestWrapper from './HardWrapPluginTestWrapper.svelte';
import { getTextContent, getTestIdValue, waitForTextContent, waitForTestId } from './test-utils';
import { test, expect } from '@playwright/experimental-ct-svelte';

/**
 * Helper to wait for paragraph count to update
 */
async function getParagraphCount(component: any): Promise<number> {
	return await getTestIdValue(component, 'paragraph-count');
}

/**
 * Wait for paragraph count to reach expected value
 */
async function waitForParagraphCount(
	component: any,
	expectedCount: number,
	timeout = 2000
): Promise<void> {
	await waitForTestId(component, 'paragraph-count', expectedCount, timeout);
}

/**
 * Wait for paragraph count to be greater than a value
 */
async function waitForParagraphCountGreaterThan(
	component: any,
	minCount: number,
	timeout = 2000
): Promise<void> {
	await expect
		.poll(async () => await getParagraphCount(component), { timeout })
		.toBeGreaterThan(minCount);
}

test.describe('HardWrapPlugin', () => {
	test('should render with initial text', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 50,
				enabled: true,
				initialText: 'This is a short line'
			}
		});

		await waitForTextContent(component, 'This is a short line');
		await waitForParagraphCount(component, 1);
	});

	test('should automatically wrap long initial text when enabled', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: 'This is a very long line that will definitely exceed the max length'
			}
		});

		// Should automatically wrap on initialization without needing to click wrap-all
		await waitForParagraphCountGreaterThan(component, 1);

		const text = await component.getByTestId('text-content').textContent();
		// Verify text is present (not lost during wrapping)
		expect(text).toContain('This is a very long line');
		expect(text).toContain('exceed');
		expect(text).toContain('length');
	});

	test('should not wrap long initial text when disabled', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: false,
				initialText: 'This is a very long line that would normally wrap but wrapping is disabled'
			}
		});

		// Should remain as single paragraph even though text exceeds maxLength
		await waitForParagraphCount(component, 1);

		const paragraphCount = await getParagraphCount(component);
		// Verify it's still just one paragraph
		expect(paragraphCount).toBe(1);

		const text = await component.getByTestId('text-content').textContent();
		// Verify all the text is present
		expect(text).toContain('This is a very long line');
		expect(text).toContain('wrapping is disabled');
	});

	test('should wrap long text with WRAP_ALL_COMMAND when enabled', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: 'This is a very long line that will definitely exceed the max length'
			}
		});

		// Trigger wrap all
		await component.getByTestId('wrap-all-button').click();

		// Should have been wrapped into multiple paragraphs
		await waitForParagraphCountGreaterThan(component, 1);
	});

	test('should not wrap when disabled', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: false,
				initialText: 'This is a very long line that would normally wrap but wrapping is disabled'
			}
		});

		// Wait for initial render
		await waitForParagraphCount(component, 1);

		// Try to trigger wrap - should not work when disabled
		await component.getByTestId('wrap-all-button').click();

		// Should remain as single paragraph when disabled
		await waitForParagraphCount(component, 1);
	});

	test('should handle WRAP_ALL_COMMAND', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 40,
				enabled: true,
				initialText:
					'This is a very long paragraph that exceeds the maximum length and should be wrapped when the wrap all command is triggered'
			}
		});

		// With auto-wrap on init, text should already be wrapped
		await waitForParagraphCountGreaterThan(component, 1);
		const initialCount = await getParagraphCount(component);

		// Click wrap all button - should be idempotent
		await component.getByTestId('wrap-all-button').click();

		// Should remain wrapped with same paragraph count
		const finalCount = await getParagraphCount(component);
		expect(finalCount).toBeGreaterThan(1);
		expect(finalCount).toBe(initialCount);
	});

	test('should not wrap markdown code blocks', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: '```javascript\nfunction veryLongFunctionNameThatExceedsMax() {}\n```'
			}
		});

		await waitForTextContent(component, '```javascript');

		// Try to trigger wrap
		await component.getByTestId('wrap-all-button').click();

		// Code blocks should remain unwrapped (though the exact behavior depends on implementation)
		await waitForTextContent(component, '```javascript');
	});

	test('should not wrap headings', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: '## This is a very long heading that exceeds maximum length'
			}
		});

		await waitForParagraphCount(component, 1);

		await component.getByTestId('wrap-all-button').click();

		// Heading should remain intact
		await waitForTextContent(
			component,
			'## This is a very long heading that exceeds maximum length'
		);
		await waitForParagraphCount(component, 1);
	});

	test('should not wrap block quotes', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: '> This is a block quote with text that exceeds maximum length'
			}
		});

		await waitForParagraphCount(component, 1);

		await component.getByTestId('wrap-all-button').click();

		await waitForTextContent(component, '> This is a block quote');
		await waitForParagraphCount(component, 1);
	});

	test('should handle multiple maxLength values', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 20,
				enabled: true,
				initialText: 'This is some text that should wrap at twenty characters per line maximum'
			}
		});

		await component.getByTestId('wrap-all-button').click();

		// Should wrap into multiple paragraphs (exact count may vary based on wrapping logic)
		await waitForParagraphCountGreaterThan(component, 1);
	});

	test('should respect empty initial text', async ({ mount }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 50,
				enabled: true,
				initialText: ''
			}
		});

		// Empty editor should have 0 paragraphs
		await waitForParagraphCount(component, 0);

		const text = await getTextContent(component);
		expect(text).toBe('');
	});

	test('should rewrap remainder when typing in middle of paragraph exceeds max length', async ({
		mount,
		page
	}) => {
		// Start with a paragraph that's close to but under the limit
		const initialText = 'This is a line that is close to the limit';
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 50,
				enabled: true,
				initialText
			}
		});

		// Wait for initial render - should be 1 paragraph
		await waitForParagraphCount(component, 1);

		// Focus the editor
		await component.getByTestId('focus-button').click();

		// Click in the middle of the text (after "line")
		const editorWrapper = component.getByTestId('editor-wrapper');
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		// Move cursor to after "line" (position 15)
		// Use keyboard shortcuts to position cursor
		await page.keyboard.press('Home'); // Go to start
		for (let i = 0; i < 15; i++) {
			await page.keyboard.press('ArrowRight');
		}

		// Type text that will cause the line to exceed 50 characters
		// Current: "This is a line that is close to the limit" (42 chars)
		// Insert " of text" (8 chars) after "line" -> "This is a line of text that is close to the limit" (50 chars)
		// Then add one more word to trigger wrapping
		await page.keyboard.type(' of additional text');

		// Wait for rewrapping to occur - should now be 2 paragraphs
		await waitForParagraphCountGreaterThan(component, 1);

		const finalText = await getTextContent(component);

		// Verify specific words to ensure nothing was dropped during wrapping
		expect(finalText).toContain('additional');
		expect(finalText).toContain('text');
		expect(finalText).toContain('close');
		expect(finalText).toContain('limit');
		expect(finalText).toContain('line');

		// Verify the paragraph count increased (text was wrapped)
		const paragraphCount = await getParagraphCount(component);
		expect(paragraphCount).toBeGreaterThan(1);
	});
});
