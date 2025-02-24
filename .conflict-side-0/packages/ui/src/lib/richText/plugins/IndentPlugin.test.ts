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
				// Now only creates one paragraph (bullet removed, cursor stays)
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				// The paragraph should be empty (bullet removed, cursor here)
				const para1 = paragraphs[0];
				if (!$isParagraphNode(para1)) throw new Error('Expected paragraph node');
				const children1 = para1.getChildren();
				expect(children1.length).toBe(0);
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
				// Now only creates one paragraph (bullet removed, cursor stays)
				const paragraphs = root.getChildren();
				expect(paragraphs.length).toBe(1);

				// The paragraph should be empty (bullet removed, cursor here)
				const para1 = paragraphs[0];
				if (!$isParagraphNode(para1)) throw new Error('Expected paragraph node');
				const children1 = para1.getChildren();
				expect(children1.length).toBe(0);
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

	describe('regression tests for wrapped bullet handling', () => {
		it('should preserve mixed tabs and spaces indentation', () => {
			editor.update(() => {
				const root = $getRoot();
				const paragraph = $createParagraphNode();
				const textNode = $createTextNode('\t  Mixed indent text');
				paragraph.append(textNode);
				root.append(paragraph);
				textNode.select(19, 19);
			});

			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				const children = root.getChildren();
				const secondLine = children[1].getTextContent();

				expect(secondLine.charCodeAt(0)).toBe(9); // tab
				expect(secondLine.charCodeAt(1)).toBe(32); // space
				expect(secondLine.charCodeAt(2)).toBe(32); // space
			});
		});

		it('should create continuation when pressing Enter in middle of wrapped bullet', () => {
			editor.update(() => {
				const root = $getRoot();

				const para1 = $createParagraphNode();
				para1.append($createTextNode('- This is a bullet that'));
				root.append(para1);

				const para2 = $createParagraphNode();
				para2.append($createTextNode('  was previously wrapped'));
				root.append(para2);

				const textNode = para1.getFirstChild();
				if ($isTextNode(textNode)) {
					textNode.select(17, 17);
				}
			});

			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				const children = root.getChildren();

				expect(children[0].getTextContent()).toBe('- This is a bulle');
				expect(children[1].getTextContent()).toBe('  t that'); // continuation, not new bullet
				expect(children[2].getTextContent()).toBe('  was previously wrapped');
			});
		});

		it('should create new bullet when at end of wrapped bullet continuation line', () => {
			editor.update(() => {
				const root = $getRoot();

				const para1 = $createParagraphNode();
				para1.append($createTextNode('- This is a bullet'));
				root.append(para1);

				const para2 = $createParagraphNode();
				para2.append($createTextNode('  that wraps'));
				root.append(para2);

				const textNode = para2.getFirstChild();
				if ($isTextNode(textNode)) {
					textNode.select(12, 12); // at end of continuation
				}
			});

			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				const children = root.getChildren();

				expect(children.length).toBe(3);
				expect(children[0].getTextContent()).toBe('- This is a bullet');
				expect(children[1].getTextContent()).toBe('  that wraps');
				expect(children[2].getTextContent()).toBe('- '); // new bullet created
			});
		});

		it('should create new numbered bullet when at end of wrapped numbered continuation', () => {
			editor.update(() => {
				const root = $getRoot();

				const para1 = $createParagraphNode();
				para1.append($createTextNode('1. First item'));
				root.append(para1);

				const para2 = $createParagraphNode();
				para2.append($createTextNode('   continues here'));
				root.append(para2);

				const textNode = para2.getFirstChild();
				if ($isTextNode(textNode)) {
					textNode.select(17, 17);
				}
			});

			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			editor.read(() => {
				const root = $getRoot();
				const children = root.getChildren();

				expect(children.length).toBe(3);
				expect(children[2].getTextContent()).toBe('2. ');
			});
		});
	});
});
