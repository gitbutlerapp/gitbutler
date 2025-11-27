import { handleEnter } from '$lib/richText/plugins/IndentPlugin.svelte';
import {
	createEditor,
	type LexicalEditor,
	COMMAND_PRIORITY_CRITICAL,
	KEY_ENTER_COMMAND
} from 'lexical';
import {
	$createTextNode,
	$createParagraphNode,
	$getRoot,
	$isTextNode,
	$isParagraphNode
} from 'lexical';
import { describe, it, expect, beforeEach } from 'vitest';

describe('IndentPlugin', () => {
	let editor: LexicalEditor;

	beforeEach(() => {
		editor = createEditor({
			namespace: 'test',
			onError: (error) => {
				throw error;
			}
		});

		// Register the handler for KEY_ENTER_COMMAND (rich text mode)
		editor.registerCommand(KEY_ENTER_COMMAND, handleEnter, COMMAND_PRIORITY_CRITICAL);
	});

	describe('indentation preservation', () => {
		it('should preserve indentation when pressing Enter', () => {
			editor.update(() => {
				const root = $getRoot();

				// In rich text mode (paragraph mode)
				// Simulate: "    Indented line"
				const para = $createParagraphNode();
				const textNode = $createTextNode('    Indented line');
				para.append(textNode);
				root.append(para);

				// Position cursor at end of line
				textNode.select(18, 18);
			});

			// Simulate pressing Enter (KEY_ENTER_COMMAND in rich text mode)
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				// In rich text mode, pressing Enter creates a NEW paragraph
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(2);

				// First paragraph should contain original text
				const para1 = paragraphs[0];
				if (!$isParagraphNode(para1)) throw new Error('Expected paragraph node');
				const children1 = para1.getChildren();
				expect(children1.length).toBe(1);
				expect($isTextNode(children1[0])).toBe(true);
				if ($isTextNode(children1[0])) {
					expect(children1[0].getTextContent()).toBe('    Indented line');
				}

				// Second paragraph should have the same indentation
				const para2 = paragraphs[1];
				if (!$isParagraphNode(para2)) throw new Error('Expected paragraph node');
				const children2 = para2.getChildren();
				expect(children2.length).toBe(1);
				expect($isTextNode(children2[0])).toBe(true);
				if ($isTextNode(children2[0])) {
					expect(children2[0].getTextContent()).toBe('    '); // 4 spaces preserved
				}
			});
		});

		it('should preserve indentation in middle of line', () => {
			editor.update(() => {
				const root = $getRoot();

				// Simulate: "    Indented line with more text"
				const para = $createParagraphNode();
				const textNode = $createTextNode('    Indented line with more text');
				para.append(textNode);
				root.append(para);

				// Position cursor after "Indented " (position 13)
				textNode.select(13, 13);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				// In rich text mode, creates two paragraphs
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(2);

				// First paragraph should have text before cursor
				const para1 = paragraphs[0];
				if (!$isParagraphNode(para1)) throw new Error('Expected paragraph node');
				const children1 = para1.getChildren();
				expect(children1.length).toBe(1);
				expect($isTextNode(children1[0])).toBe(true);
				if ($isTextNode(children1[0])) {
					expect(children1[0].getTextContent()).toBe('    Indented ');
				}

				// Second paragraph should have indent + remainder
				const para2 = paragraphs[1];
				if (!$isParagraphNode(para2)) throw new Error('Expected paragraph node');
				const children2 = para2.getChildren();
				expect(children2.length).toBe(1);
				expect($isTextNode(children2[0])).toBe(true);
				if ($isTextNode(children2[0])) {
					expect(children2[0].getTextContent()).toBe('    line with more text');
				}
			});
		});
	});

	describe('bullet point handling', () => {
		it('should increment numbered bullet points', () => {
			editor.update(() => {
				const root = $getRoot();

				// Simulate: "1. First item"
				const para = $createParagraphNode();
				const textNode = $createTextNode('1. First item');
				para.append(textNode);
				root.append(para);

				// Position cursor at end
				textNode.select(13, 13);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				// In rich text mode, creates two paragraphs
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(2);

				// First paragraph
				const para1 = paragraphs[0];
				if (!$isParagraphNode(para1)) throw new Error('Expected paragraph node');

				// Second paragraph should have "2. "
				const para2 = paragraphs[1];
				if (!$isParagraphNode(para2)) throw new Error('Expected paragraph node');
				const children2 = para2.getChildren();
				expect(children2.length).toBe(1);
				expect($isTextNode(children2[0])).toBe(true);
				if ($isTextNode(children2[0])) {
					expect(children2[0].getTextContent()).toBe('2. ');
				}
			});
		});

		it('should preserve bullet style', () => {
			editor.update(() => {
				const root = $getRoot();

				// Simulate: "- First item"
				const para = $createParagraphNode();
				const textNode = $createTextNode('- First item');
				para.append(textNode);
				root.append(para);

				// Position cursor at end
				textNode.select(12, 12);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				// In rich text mode, creates two paragraphs
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(2);

				// Second paragraph should have "- "
				const para2 = paragraphs[1];
				if (!$isParagraphNode(para2)) throw new Error('Expected paragraph node');
				const children2 = para2.getChildren();
				expect(children2.length).toBe(1);
				expect($isTextNode(children2[0])).toBe(true);
				if ($isTextNode(children2[0])) {
					expect(children2[0].getTextContent()).toBe('- ');
				}
			});
		});

		it('should remove empty bullet when pressing Enter', () => {
			editor.update(() => {
				const root = $getRoot();

				// Simulate: "- "
				const para = $createParagraphNode();
				const textNode = $createTextNode('- ');
				para.append(textNode);
				root.append(para);

				// Position cursor at end
				textNode.select(2, 2);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				// In rich text mode, creates two paragraphs
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(2);

				// First paragraph should be empty (bullet removed)
				const para1 = paragraphs[0];
				if (!$isParagraphNode(para1)) throw new Error('Expected paragraph node');
				const children1 = para1.getChildren();
				expect(children1.length).toBe(0);

				// Second paragraph should also be empty (cursor position)
				const para2 = paragraphs[1];
				if (!$isParagraphNode(para2)) throw new Error('Expected paragraph node');
				const children2 = para2.getChildren();
				expect(children2.length).toBe(0);
			});
		});

		it('should remove empty numbered bullet when pressing Enter', () => {
			editor.update(() => {
				const root = $getRoot();

				// Simulate: "1. "
				const para = $createParagraphNode();
				const textNode = $createTextNode('1. ');
				para.append(textNode);
				root.append(para);

				// Position cursor at end
				textNode.select(3, 3);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				// In rich text mode, creates two paragraphs
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(2);

				// First paragraph should be empty (bullet removed)
				const para1 = paragraphs[0];
				if (!$isParagraphNode(para1)) throw new Error('Expected paragraph node');
				const children1 = para1.getChildren();
				expect(children1.length).toBe(0);

				// Second paragraph should also be empty (cursor position)
				const para2 = paragraphs[1];
				if (!$isParagraphNode(para2)) throw new Error('Expected paragraph node');
				const children2 = para2.getChildren();
				expect(children2.length).toBe(0);
			});
		});
	});

	describe('complex scenarios', () => {
		it('should handle Enter in middle of indented line', () => {
			editor.update(() => {
				const root = $getRoot();

				// In rich text mode: two paragraphs
				// "First line" (paragraph 1)
				// "    Indented second line" (paragraph 2)
				const para1 = $createParagraphNode();
				const text1 = $createTextNode('First line');
				para1.append(text1);
				root.append(para1);

				const para2 = $createParagraphNode();
				const text2 = $createTextNode('    Indented second line');
				para2.append(text2);
				root.append(para2);

				// Position cursor in middle of "Indented second line" after "Indented " (position 13)
				text2.select(13, 13);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				// Should now have 3 paragraphs
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(3);

				// First paragraph unchanged
				const p1 = paragraphs[0];
				if (!$isParagraphNode(p1)) throw new Error('Expected paragraph node');
				const c1 = p1.getChildren();
				expect(c1.length).toBe(1);
				if ($isTextNode(c1[0])) {
					expect(c1[0].getTextContent()).toBe('First line');
				}

				// Second paragraph has text before cursor
				const p2 = paragraphs[1];
				if (!$isParagraphNode(p2)) throw new Error('Expected paragraph node');
				const c2 = p2.getChildren();
				expect(c2.length).toBe(1);
				if ($isTextNode(c2[0])) {
					expect(c2[0].getTextContent()).toBe('    Indented ');
				}

				// Third paragraph has indented text after cursor
				const p3 = paragraphs[2];
				if (!$isParagraphNode(p3)) throw new Error('Expected paragraph node');
				const c3 = p3.getChildren();
				expect(c3.length).toBe(1);
				if ($isTextNode(c3[0])) {
					expect(c3[0].getTextContent()).toBe('    second line');
				}
			});
		});
	});
});
