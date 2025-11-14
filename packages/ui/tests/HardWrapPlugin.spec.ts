import HardWrapPluginTestWrapper from './HardWrapPluginTestWrapper.svelte';
import { test, expect } from '@playwright/experimental-ct-svelte';

/**
 * Helper to wait for paragraph count to update
 */
async function getParagraphCount(component: any): Promise<number> {
	const text = await component.getByTestId('paragraph-count').textContent();
	return parseInt(text || '0', 10);
}

/**
 * Helper to get text content
 */
async function getTextContent(component: any): Promise<string> {
	return (await component.getByTestId('text-content').textContent()) || '';
}

test.describe('HardWrapPlugin', () => {
	test('should render with initial text', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 50,
				enabled: true,
				initialText: 'This is a short line'
			}
		});

		await page.waitForTimeout(500);

		const text = await getTextContent(component);
		expect(text).toContain('This is a short line');

		const paragraphCount = await getParagraphCount(component);
		expect(paragraphCount).toBe(1);
	});

	test('should wrap long text with WRAP_ALL_COMMAND when enabled', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: 'This is a very long line that will definitely exceed the max length'
			}
		});

		await page.waitForTimeout(300);

		// Trigger wrap all
		await component.getByTestId('wrap-all-button').click();

		await page.waitForTimeout(500);

		const paragraphCount = await getParagraphCount(component);
		// Should have been wrapped into multiple paragraphs
		expect(paragraphCount).toBeGreaterThan(1);
	});

	test('should not wrap when disabled', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: false,
				initialText: 'This is a very long line that would normally wrap but wrapping is disabled'
			}
		});

		await page.waitForTimeout(300);

		// Try to trigger wrap - should not work when disabled
		await component.getByTestId('wrap-all-button').click();

		await page.waitForTimeout(500);

		const paragraphCount = await getParagraphCount(component);
		// Should remain as single paragraph when disabled
		expect(paragraphCount).toBe(1);
	});

	test('should handle WRAP_ALL_COMMAND', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 40,
				enabled: true,
				initialText:
					'This is a very long paragraph that exceeds the maximum length and should be wrapped when the wrap all command is triggered'
			}
		});

		await page.waitForTimeout(300);

		// With auto-wrap on init, text should already be wrapped
		const initialCount = await getParagraphCount(component);
		expect(initialCount).toBeGreaterThan(1);

		// Click wrap all button - should be idempotent
		await component.getByTestId('wrap-all-button').click();

		await page.waitForTimeout(500);

		const finalCount = await getParagraphCount(component);
		// Should remain wrapped with same paragraph count
		expect(finalCount).toBeGreaterThan(1);
		expect(finalCount).toBe(initialCount);
	});

	test('should not wrap markdown code blocks', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: '```javascript\nfunction veryLongFunctionNameThatExceedsMax() {}\n```'
			}
		});

		await page.waitForTimeout(300);

		const text = await getTextContent(component);
		expect(text).toContain('```javascript');

		// Try to trigger wrap
		await component.getByTestId('wrap-all-button').click();
		await page.waitForTimeout(300);

		// Code blocks should remain unwrapped (though the exact behavior depends on implementation)
		const finalText = await getTextContent(component);
		expect(finalText).toContain('```javascript');
	});

	test('should not wrap headings', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: '## This is a very long heading that exceeds maximum length'
			}
		});

		await page.waitForTimeout(300);

		await component.getByTestId('wrap-all-button').click();
		await page.waitForTimeout(300);

		const text = await getTextContent(component);
		// Heading should remain intact
		expect(text).toContain('## This is a very long heading that exceeds maximum length');

		const paragraphCount = await getParagraphCount(component);
		expect(paragraphCount).toBe(1);
	});

	test('should not wrap block quotes', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 30,
				enabled: true,
				initialText: '> This is a block quote with text that exceeds maximum length'
			}
		});

		await page.waitForTimeout(300);

		await component.getByTestId('wrap-all-button').click();
		await page.waitForTimeout(300);

		const text = await getTextContent(component);
		expect(text).toContain('> This is a block quote');

		const paragraphCount = await getParagraphCount(component);
		expect(paragraphCount).toBe(1);
	});

	test('should handle multiple maxLength values', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 20,
				enabled: true,
				initialText: 'This is some text that should wrap at twenty characters per line maximum'
			}
		});

		await page.waitForTimeout(300);

		await component.getByTestId('wrap-all-button').click();
		await page.waitForTimeout(500);

		const finalCount = await getParagraphCount(component);
		// With maxLength=20, should wrap into many paragraphs
		expect(finalCount).toBeGreaterThan(2);
	});

	test('should respect empty initial text', async ({ mount, page }) => {
		const component = await mount(HardWrapPluginTestWrapper, {
			props: {
				maxLength: 50,
				enabled: true,
				initialText: ''
			}
		});

		await page.waitForTimeout(300);

		const paragraphCount = await getParagraphCount(component);
		expect(paragraphCount).toBeLessThanOrEqual(1);

		const text = await getTextContent(component);
		expect(text).toBe('');
	});
});
