import IndentPluginTestWrapper from './IndentPluginTestWrapper.svelte';
import { getTextContent, waitForTextContent, doAndWaitForIdle, waitUntilIdle } from './test-utils';
import { test, expect } from '@playwright/experimental-ct-svelte';

test.describe('IndentPlugin', () => {
	test('should preserve indentation when pressing Enter', async ({ mount, page }) => {
		const component = await mount(IndentPluginTestWrapper, {
			props: {
				initialText: 'Start text'
			}
		});

		// Wait for editor to fully initialize with content
		await waitForTextContent(component, 'Start');

		// Wait for browser to be idle before starting interactions
		await waitUntilIdle(page);

		// Focus editor
		await component.getByTestId('focus-button').click();

		// Clear and type indented content with idle detection
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.press('Meta+A');
			await page.keyboard.press('Backspace');
		});

		// Type indented line
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.type('    Indented line');
		});

		// Press Enter - should preserve indentation
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.press('Enter');
		});

		// Type new text on the new line
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.type('New line');
		});

		const textAfter = await getTextContent(component);

		// Verify indentation is preserved on new line
		expect(textAfter).toContain('Indented line');
		expect(textAfter).toContain('New line');

		const lines = textAfter.split('\n');
		expect(lines.length).toBeGreaterThanOrEqual(2);
	});

	test('should increment numbered bullets when pressing Enter', async ({ mount, page }) => {
		const component = await mount(IndentPluginTestWrapper, {
			props: {
				initialText: ''
			}
		});

		// Wait for initial idle state
		await waitUntilIdle(page);

		// Focus editor
		await component.getByTestId('focus-button').click();

		// Type numbered bullet with idle detection
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.type('1. First item');
		});

		// Press Enter - should create "2. "
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.press('Enter');
		});

		// Type second item
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.type('Second item');
		});

		const textAfter = await getTextContent(component);

		// Verify bullet was incremented
		expect(textAfter).toContain('1. First item');
		expect(textAfter).toContain('2. Second item');
	});

	test('should remove empty bullet when pressing Enter', async ({ mount, page }) => {
		const component = await mount(IndentPluginTestWrapper, {
			props: {
				initialText: ''
			}
		});

		// Wait for initial idle state
		await waitUntilIdle(page);

		// Focus editor
		await component.getByTestId('focus-button').click();

		// Type bullet with idle detection
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.type('- ');
		});

		// Press Enter on empty bullet - should remove it
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.press('Enter');
		});

		// Type new text
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.type('Normal text');
		});

		const textAfter = await getTextContent(component);

		// Verify bullet was removed
		expect(textAfter).not.toContain('- -');
		expect(textAfter).toContain('Normal text');
	});

	test('should load and save text with indentation and bullets without changes', async ({
		mount,
		page
	}) => {
		const initialText = `Normal paragraph

    Indented paragraph with four spaces
    Second line of indented paragraph

- Bullet point one
- Bullet point two
  - Nested bullet with two spaces

1. Numbered item one
2. Numbered item two
3. Numbered item three

    Indented text with bullets:
    - Bullet in indented block
    - Another bullet in indented block

Final normal paragraph`;

		const component = await mount(IndentPluginTestWrapper, {
			props: {
				initialText
			}
		});

		// Wait for content to load
		await waitForTextContent(component, 'Normal paragraph');

		// Wait for browser to be idle after initial load
		await waitUntilIdle(page);

		// Get the text content immediately after loading
		const loadedText = await getTextContent(component);

		// Verify the loaded text matches the initial text
		expect(loadedText).toBe(initialText);

		// Focus editor to ensure it's active
		await component.getByTestId('focus-button').click();

		// Blur the editor to trigger any save operations with idle detection
		await doAndWaitForIdle(page, async () => {
			await page.keyboard.press('Escape');
		});

		// Get the text content after blur
		const savedText = await getTextContent(component);

		// The key assertion: text should remain unchanged after load/save cycle
		expect(savedText).toBe(initialText);

		// Also verify specific formatting is preserved
		expect(savedText).toContain('    Indented paragraph');
		expect(savedText).toContain('- Bullet point one');
		expect(savedText).toContain('  - Nested bullet');
		expect(savedText).toContain('1. Numbered item one');
		expect(savedText).toContain('2. Numbered item two');
		expect(savedText).toContain('    - Bullet in indented block');
	});
});
