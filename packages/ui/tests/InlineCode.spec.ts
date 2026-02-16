import InlineCodeTestWrapper from "./InlineCodeTestWrapper.svelte";
import { getTextContent, waitForTextContent, waitForTestId } from "./test-utils";
import { test, expect } from "@playwright/experimental-ct-svelte";
import type { Locator } from "playwright";

async function getInlineCodeCount(component: Locator): Promise<number> {
	const text = await component.getByTestId("inline-code-count").textContent();
	return parseInt(text || "0", 10);
}

async function waitForInlineCodeCount(
	component: Locator,
	expectedCount: number,
	timeout = 2000,
): Promise<void> {
	await waitForTestId(component, "inline-code-count", expectedCount, timeout);
}

test.describe("InlineCode", () => {
	test("should create an inline code node when typing backtick-wrapped text", async ({
		mount,
		page,
	}) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		// Focus the editor
		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		// Type backtick-wrapped text: `hello`
		await page.keyboard.type("`hello`");

		// Wait for the inline code node to be created
		await waitForInlineCodeCount(component, 1);

		const text = await getTextContent(component);
		expect(text).toContain("`hello`");
	});

	test("should render inline code with code element in the DOM", async ({ mount, page }) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		await page.keyboard.type("`code`");

		await waitForInlineCodeCount(component, 1);

		// Verify that a <code> element with the inline-code class exists in the DOM
		const codeElement = editorWrapper.locator("code.inline-code");
		await expect(codeElement).toBeVisible();
		await expect(codeElement).toHaveText("`code`");
	});

	test("should revert to plain text when closing backtick is deleted via backspace", async ({
		mount,
		page,
	}) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		// Type text with inline code
		await page.keyboard.type("before `code`");

		await waitForInlineCodeCount(component, 1);

		// Backspace deletes the closing backtick, which reverts the node to plain text.
		await page.keyboard.press("Backspace");

		await waitForInlineCodeCount(component, 0);

		// The inline code node should be gone
		const codeElement = editorWrapper.locator("code.inline-code");
		await expect(codeElement).toHaveCount(0);
	});

	test("should allow editing text inside inline code node", async ({ mount, page }) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		// Create an inline code node
		await page.keyboard.type("`hello`");

		await waitForInlineCodeCount(component, 1);

		// Move cursor into the inline code node: Home then right-arrow into it
		await page.keyboard.press("Home");
		// Arrow right past the opening backtick and 'h'
		await page.keyboard.press("ArrowRight");
		await page.keyboard.press("ArrowRight");

		// Type additional text inside the node (between ` and e -> `hXello`)
		await page.keyboard.type("X");

		// The node should still be an inline code node (backticks are still present)
		await waitForInlineCodeCount(component, 1);

		// The code element should still be visible
		const codeElement = editorWrapper.locator("code.inline-code");
		await expect(codeElement).toBeVisible();
	});

	test("should create inline code in the middle of text", async ({ mount, page }) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		await page.keyboard.type("use `useState` here");

		await waitForInlineCodeCount(component, 1);
		await waitForTextContent(component, "here");

		const text = await getTextContent(component);
		expect(text).toContain("use");
		expect(text).toContain("`useState`");
		expect(text).toContain("here");
	});

	test("should create multiple inline code nodes", async ({ mount, page }) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		await page.keyboard.type("`foo` and `bar`");

		await waitForInlineCodeCount(component, 2);

		const text = await getTextContent(component);
		expect(text).toContain("`foo`");
		expect(text).toContain("`bar`");
	});

	test("should re-create inline code node when backtick is restored after deletion", async ({
		mount,
		page,
	}) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		// Create an inline code node
		await page.keyboard.type("`code`");
		await waitForInlineCodeCount(component, 1);

		// Delete the closing backtick
		await page.keyboard.press("Backspace");
		await waitForInlineCodeCount(component, 0);

		// Re-type the closing backtick — should re-create the inline code node
		await page.keyboard.type("`");
		await waitForInlineCodeCount(component, 1);

		const codeElement = editorWrapper.locator("code.inline-code");
		await expect(codeElement).toBeVisible();
	});

	test("should re-create inline code node when opening backtick is restored after deletion", async ({
		mount,
		page,
		browserName,
	}) => {
		// On WebKit, Home+ArrowRight navigation into the inline code node
		// positions the cursor differently, so this test only runs on Chromium.
		test.skip(browserName !== "chromium", "Cursor navigation into inline code differs on WebKit");

		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		// Create an inline code node
		await page.keyboard.type("`code`");
		await waitForInlineCodeCount(component, 1);

		// Navigate into the inline code node and delete the opening backtick.
		// Home puts cursor at start of line; ArrowRight moves into the node.
		// Backspace deletes the opening backtick.
		await page.keyboard.press("Home");
		await page.keyboard.press("ArrowRight");
		await page.keyboard.press("Backspace");
		await waitForInlineCodeCount(component, 0);

		// Re-type the opening backtick — the node transform approach (unlike
		// the old text-match transformer) checks the entire TextNode content,
		// so it detects the backtick pattern regardless of cursor position.
		await page.keyboard.type("`");
		await waitForInlineCodeCount(component, 1);

		const codeElement = editorWrapper.locator("code.inline-code");
		await expect(codeElement).toBeVisible();
	});

	test("should not create inline code for unmatched backtick", async ({ mount, page }) => {
		const component = await mount(InlineCodeTestWrapper, {
			props: { initialText: "" },
		});

		await component.getByTestId("focus-button").click();

		const editorWrapper = component.getByTestId("editor-wrapper");
		const contentEditable = editorWrapper.locator('[contenteditable="true"]').first();
		await contentEditable.click();

		await page.keyboard.type("this has a ` single backtick");

		// Wait a moment for any potential transformation
		await page.waitForTimeout(300);

		const count = await getInlineCodeCount(component);
		expect(count).toBe(0);
	});
});
