import { handleEnter } from '$lib/richText/plugins/PlainTextIndentPlugin.svelte';
import { createEditor, type LexicalEditor, COMMAND_PRIORITY_CRITICAL } from 'lexical';
import {
	$createTextNode,
	$createLineBreakNode,
	$createParagraphNode,
	$getRoot,
	$isTextNode,
	$isLineBreakNode,
	$isParagraphNode,
	INSERT_LINE_BREAK_COMMAND
} from 'lexical';
import { describe, it, expect, beforeEach } from 'vitest';

describe('PlainTextIndentPlugin', () => {
	let editor: LexicalEditor;

	beforeEach(() => {
		editor = createEditor({
			namespace: 'test',
			onError: (error) => {
				throw error;
			}
		});

		// Register the handler
		editor.registerCommand(INSERT_LINE_BREAK_COMMAND, handleEnter, COMMAND_PRIORITY_CRITICAL);
	});

	describe('indentation preservation', () => {
		it('should preserve indentation when pressing Enter', () => {
			editor.update(() => {
				const root = $getRoot();

				// In plaintext mode, text nodes must be inside a paragraph
				// Simulate: "    Indented line"
				const para = $createParagraphNode();
				const textNode = $createTextNode('    Indented line');
				para.append(textNode);
				root.append(para);

				// Position cursor at end of line
				textNode.select(18, 18);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false);
			});

			editor.read(() => {
				const root = $getRoot();
				// In plaintext mode, there's still a paragraph containing text + linebreak + text
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				const para = paragraphs[0];
				if (!$isParagraphNode(para)) throw new Error('Expected paragraph node');
				const children = para.getChildren();

				// Should have: TextNode + LineBreakNode + TextNode (with indent)
				expect(children.length).toBe(3);
				expect($isTextNode(children[0])).toBe(true);
				expect($isLineBreakNode(children[1])).toBe(true);
				expect($isTextNode(children[2])).toBe(true);

				// The new text node should start with the same indentation
				const newTextNode = children[2];
				if ($isTextNode(newTextNode)) {
					expect(newTextNode.getTextContent()).toBe('    '); // 4 spaces
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
				editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false);
			});

			editor.read(() => {
				const root = $getRoot();
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				const para = paragraphs[0];
				if (!$isParagraphNode(para)) throw new Error('Expected paragraph node');
				const children = para.getChildren();

				expect(children.length).toBe(3);

				// First text node should have text before cursor
				const firstText = children[0];
				if ($isTextNode(firstText)) {
					expect(firstText.getTextContent()).toBe('    Indented ');
				}

				// LineBreak
				expect($isLineBreakNode(children[1])).toBe(true);

				// Second text node should have indent + remainder
				const secondText = children[2];
				if ($isTextNode(secondText)) {
					expect(secondText.getTextContent()).toBe('    line with more text');
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
				editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false);
			});

			editor.read(() => {
				const root = $getRoot();
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				const para = paragraphs[0];
				if (!$isParagraphNode(para)) throw new Error('Expected paragraph node');
				const children = para.getChildren();

				expect(children.length).toBe(3);

				// New text node should have "2. "
				const newTextNode = children[2];
				if ($isTextNode(newTextNode)) {
					expect(newTextNode.getTextContent()).toBe('2. ');
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
				editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false);
			});

			editor.read(() => {
				const root = $getRoot();
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				const para = paragraphs[0];
				if (!$isParagraphNode(para)) throw new Error('Expected paragraph node');
				const children = para.getChildren();

				expect(children.length).toBe(3);

				// New text node should have "- "
				const newTextNode = children[2];
				if ($isTextNode(newTextNode)) {
					expect(newTextNode.getTextContent()).toBe('- ');
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
				editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false);
			});

			editor.read(() => {
				const root = $getRoot();
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				const para = paragraphs[0];
				if (!$isParagraphNode(para)) throw new Error('Expected paragraph node');
				const children = para.getChildren();

				// Empty bullet should be removed, leaving an empty paragraph
				expect(children.length).toBe(0);
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
				editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false);
			});

			editor.read(() => {
				const root = $getRoot();
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				const para = paragraphs[0];
				if (!$isParagraphNode(para)) throw new Error('Expected paragraph node');
				const children = para.getChildren();

				// Empty bullet should be removed, leaving an empty paragraph
				expect(children.length).toBe(0);
			});
		});
	});

	describe('complex scenarios', () => {
		it('should handle Enter in middle of indented line with existing line breaks', () => {
			editor.update(() => {
				const root = $getRoot();

				// Simulate plaintext with existing line breaks:
				// "First line\n    Indented second line"
				const para = $createParagraphNode();
				const text1 = $createTextNode('First line');
				const lineBreak = $createLineBreakNode();
				const text2 = $createTextNode('    Indented second line');

				para.append(text1);
				para.append(lineBreak);
				para.append(text2);
				root.append(para);

				// Position cursor in middle of "Indented second line" after "Indented " (position 13)
				text2.select(13, 13);
			});

			// Simulate pressing Enter
			editor.update(() => {
				editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false);
			});

			editor.read(() => {
				const root = $getRoot();
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				const para = paragraphs[0];
				if (!$isParagraphNode(para)) throw new Error('Expected paragraph node');
				const children = para.getChildren();

				// Should have: text1, lineBreak, text2 (before cursor), new lineBreak, text3 (after cursor with indent)
				expect(children.length).toBe(5);

				// Verify structure
				expect($isTextNode(children[0])).toBe(true); // "First line"
				expect($isLineBreakNode(children[1])).toBe(true);
				expect($isTextNode(children[2])).toBe(true); // "    Indented "
				expect($isLineBreakNode(children[3])).toBe(true); // New line break
				expect($isTextNode(children[4])).toBe(true); // "    second line"

				// Verify indentation preserved
				const newTextNode = children[4];
				if ($isTextNode(newTextNode)) {
					expect(newTextNode.getTextContent()).toBe('    second line');
				}
			});
		});
	});
});
