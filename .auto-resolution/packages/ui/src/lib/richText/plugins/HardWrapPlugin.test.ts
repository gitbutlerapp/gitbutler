import { wrapIfNecessary } from '$lib/richText/linewrap';
import { handleEnter } from '$lib/richText/plugins/IndentPlugin.svelte';
import {
	createEditor,
	TextNode,
	type LexicalEditor,
	COMMAND_PRIORITY_CRITICAL,
	KEY_ENTER_COMMAND,
	type NodeKey,
	type NodeMutation
} from 'lexical';
import {
	$createParagraphNode as createParagraphNode,
	$createTextNode as createTextNode,
	$getRoot as getRoot,
	$getNodeByKey as getNodeByKey,
	$getSelection as getSelection,
	$isRangeSelection as isRangeSelection,
	$isTextNode as isTextNode,
	type TextNode as LexicalTextNode
} from 'lexical';
import { describe, it, expect, beforeEach } from 'vitest';

describe('HardWrapPlugin with multi-paragraph structure', () => {
	let editor: LexicalEditor;

	beforeEach(() => {
		editor = createEditor({
			namespace: 'test',
			onError: (error) => {
				throw error;
			}
		});
	});

	it('should wrap a single long line into multiple paragraphs', () => {
		editor.update(() => {
			const root = getRoot();
			const paragraph = createParagraphNode();
			const textNode = createTextNode('This is a very long line that exceeds the maximum length');
			paragraph.append(textNode);
			root.append(paragraph);

			// Trigger wrapping with maxLength of 20
			wrapIfNecessary({ node: textNode, maxLength: 20 });
		});

		editor.read(() => {
			const root = getRoot();
			const children = root.getChildren();

			// Should have created multiple paragraphs
			expect(children.length).toBeGreaterThan(1);
		});
	});

	it('should handle wrapping text with indentation', () => {
		editor.update(() => {
			const root = getRoot();
			const paragraph = createParagraphNode();
			const textNode = createTextNode('  This is an indented line that is very long');
			paragraph.append(textNode);
			root.append(paragraph);

			wrapIfNecessary({ node: textNode, maxLength: 20 });
		});

		editor.read(() => {
			const root = getRoot();
			const children = root.getChildren();

			// Check that indentation is preserved
			if (children.length > 1) {
				const secondPara = children[1];
				const text = secondPara.getTextContent();
				expect(text.startsWith('  ')).toBe(true);
			}
		});
	});

	it('should handle wrapping bullet points', () => {
		editor.update(() => {
			const root = getRoot();
			const paragraph = createParagraphNode();
			const textNode = createTextNode(
				'- This is a bullet point that is very long and needs wrapping'
			);
			paragraph.append(textNode);
			root.append(paragraph);

			wrapIfNecessary({ node: textNode, maxLength: 25 });
		});

		editor.read(() => {
			const root = getRoot();
			const children = root.getChildren();

			// First line should have bullet
			expect(children[0].getTextContent()).toContain('-');

			// Subsequent lines should be indented
			if (children.length > 1) {
				const secondPara = children[1];
				const text = secondPara.getTextContent();
				expect(text.startsWith('  ')).toBe(true);
			}
		});
	});

	it('should not wrap lines that are exempt (code blocks)', () => {
		editor.update(() => {
			const root = getRoot();
			const paragraph = createParagraphNode();
			const textNode = createTextNode('```typescript');
			paragraph.append(textNode);
			root.append(paragraph);

			wrapIfNecessary({ node: textNode, maxLength: 10 });
		});

		editor.read(() => {
			const root = getRoot();
			const children = root.getChildren();

			// Should not have wrapped
			expect(children.length).toBe(1);
			expect(children[0].getTextContent()).toBe('```typescript');
		});
	});

	it('should not wrap lines shorter than maxLength', () => {
		editor.update(() => {
			const root = getRoot();
			const paragraph = createParagraphNode();
			const textNode = createTextNode('Short line');
			paragraph.append(textNode);
			root.append(paragraph);

			wrapIfNecessary({ node: textNode, maxLength: 50 });
		});

		editor.read(() => {
			const root = getRoot();
			const children = root.getChildren();

			// Should not have wrapped
			expect(children.length).toBe(1);
			expect(children[0].getTextContent()).toBe('Short line');
		});
	});

	it('should handle text without spaces', () => {
		editor.update(() => {
			const root = getRoot();
			const paragraph = createParagraphNode();
			const textNode = createTextNode('verylongtextwithoutspaces');
			paragraph.append(textNode);
			root.append(paragraph);

			wrapIfNecessary({ node: textNode, maxLength: 10 });
		});

		editor.read(() => {
			const root = getRoot();
			const children = root.getChildren();

			// Should not wrap text without spaces
			expect(children.length).toBe(1);
		});
	});

	describe('cursor positioning', () => {
		it('should maintain cursor position when editing at the start', () => {
			let textNodeKey = '';

			editor.update(() => {
				const root = getRoot();
				const paragraph = createParagraphNode();
				const textNode = createTextNode('This is a very long line that needs wrapping');
				paragraph.append(textNode);
				root.append(paragraph);
				textNodeKey = textNode.getKey();

				// Set cursor at position 5 (after "This ")
				textNode.select(5, 5);
			});

			editor.update(() => {
				const node = getNodeByKey(textNodeKey) as LexicalTextNode;
				wrapIfNecessary({ node, maxLength: 20 });
			});

			editor.read(() => {
				const selection = getSelection();
				if (isRangeSelection(selection)) {
					const anchorNode = selection.anchor.getNode();
					const offset = selection.anchor.offset;

					// Cursor should still be in the first paragraph
					expect(anchorNode.getTextContent()).toContain('This is a very long');
					expect(offset).toBe(5);
				}
			});
		});

		it('should move cursor to next paragraph when editing near wrap point', () => {
			let textNodeKey = '';

			editor.update(() => {
				const root = getRoot();
				const paragraph = createParagraphNode();
				const textNode = createTextNode('Short text that will become long text after edit');
				paragraph.append(textNode);
				root.append(paragraph);
				textNodeKey = textNode.getKey();

				// Set cursor at position 35 (towards the end)
				textNode.select(35, 35);
			});

			editor.update(() => {
				const node = getNodeByKey(textNodeKey) as LexicalTextNode;
				wrapIfNecessary({ node, maxLength: 25 });
			});

			editor.read(() => {
				const selection = getSelection();
				const root = getRoot();
				const children = root.getChildren();

				if (isRangeSelection(selection)) {
					// Cursor should have moved to a subsequent paragraph
					// since position 35 is past the first wrap point
					expect(children.length).toBeGreaterThan(1);
				}
			});
		});

		it('should handle cursor at the very end of wrapped text', () => {
			let textNodeKey = '';

			editor.update(() => {
				const root = getRoot();
				const paragraph = createParagraphNode();
				const textNode = createTextNode('This is a very long line that definitely needs wrapping');
				paragraph.append(textNode);
				root.append(paragraph);
				textNodeKey = textNode.getKey();

				// Set cursor at the end
				const length = textNode.getTextContentSize();
				textNode.select(length, length);
			});

			editor.update(() => {
				const node = getNodeByKey(textNodeKey) as LexicalTextNode;
				wrapIfNecessary({ node, maxLength: 20 });
			});

			editor.read(() => {
				const selection = getSelection();
				const root = getRoot();
				const children = root.getChildren();

				if (isRangeSelection(selection)) {
					const anchorNode = selection.anchor.getNode();

					// Cursor should be in the last paragraph
					const lastPara = children[children.length - 1];
					expect(anchorNode.getParent()?.getKey()).toBe(lastPara.getKey());
				}
			});
		});
	});

	describe('rewrapping paragraphs', () => {
		it('should rewrap multiple related paragraphs when editing the first one', () => {
			editor.update(() => {
				const root = getRoot();

				// Create multiple paragraphs that look like they were previously wrapped
				const para1 = createParagraphNode();
				para1.append(createTextNode('This is the first'));
				root.append(para1);

				const para2 = createParagraphNode();
				para2.append(createTextNode('line of a long'));
				root.append(para2);

				const para3 = createParagraphNode();
				para3.append(createTextNode('paragraph text'));
				root.append(para3);

				// Now edit the first paragraph to make it longer
				const textNode = para1.getFirstChild() as LexicalTextNode;
				textNode.setTextContent('This is the first line that has been made much longer');

				// Trigger rewrap
				wrapIfNecessary({ node: textNode, maxLength: 20 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should have rewrapped all related paragraphs
				expect(children.length).toBeGreaterThan(1);

				// Each paragraph should respect the maxLength
				children.forEach((child) => {
					const text = child.getTextContent();
					// Allow some tolerance for word boundaries
					expect(text.length).toBeLessThanOrEqual(25);
				});
			});
		});

		it('should handle editing in the middle of a long unwrapped line', () => {
			let textNodeKey = '';

			editor.update(() => {
				const root = getRoot();
				const paragraph = createParagraphNode();
				const textNode = createTextNode(
					'This is a very long line that has not been wrapped yet and needs to be wrapped'
				);
				paragraph.append(textNode);
				root.append(paragraph);
				textNodeKey = textNode.getKey();

				// Set cursor towards the end (position 60)
				textNode.select(60, 60);
			});

			editor.update(() => {
				const node = getNodeByKey(textNodeKey) as LexicalTextNode;
				// Simulate adding text by modifying the content
				node.setTextContent(
					'This is a very long line that has not been wrapped yet and definitely needs to be wrapped now'
				);

				wrapIfNecessary({ node, maxLength: 25 });
			});

			editor.read(() => {
				const selection = getSelection();
				const root = getRoot();
				const children = root.getChildren();

				// Should have created multiple paragraphs
				expect(children.length).toBeGreaterThan(2);

				if (isRangeSelection(selection)) {
					const anchorNode = selection.anchor.getNode();

					// Cursor should be in one of the later paragraphs since edit was at position 60
					const anchorParent = anchorNode.getParent();
					const parentIndex = children.findIndex(
						(child) => child.getKey() === anchorParent?.getKey()
					);

					// Should be in the last few paragraphs
					expect(parentIndex).toBeGreaterThan(0);
				}
			});
		});

		it('should not rewrap paragraphs with different indentation', () => {
			editor.update(() => {
				const root = getRoot();

				// First paragraph with no indentation
				const para1 = createParagraphNode();
				para1.append(createTextNode('This is a long line that needs wrapping and has no indent'));
				root.append(para1);

				// Second paragraph with indentation (should not be considered related)
				const para2 = createParagraphNode();
				para2.append(createTextNode('  This line has indent'));
				root.append(para2);

				// Trigger wrap on first paragraph
				const textNode = para1.getFirstChild() as LexicalTextNode;
				wrapIfNecessary({ node: textNode, maxLength: 20 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should have wrapped the first paragraph but not touched the indented one
				const lastPara = children[children.length - 1];
				expect(lastPara.getTextContent()).toBe('  This line has indent');
			});
		});

		it('should stop rewrapping at bullet points', () => {
			editor.update(() => {
				const root = getRoot();

				// First paragraph
				const para1 = createParagraphNode();
				para1.append(
					createTextNode('This is a long line that needs wrapping before the bullet point')
				);
				root.append(para1);

				// Second paragraph with a bullet (should not be rewrapped)
				const para2 = createParagraphNode();
				para2.append(createTextNode('- This is a bullet point'));
				root.append(para2);

				// Trigger wrap on first paragraph
				const textNode = para1.getFirstChild() as LexicalTextNode;
				wrapIfNecessary({ node: textNode, maxLength: 20 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should have wrapped the first paragraph but kept bullet separate
				const lastPara = children[children.length - 1];
				expect(lastPara.getTextContent()).toBe('- This is a bullet point');
			});
		});

		it('should preserve empty paragraphs as boundaries', () => {
			editor.update(() => {
				const root = getRoot();

				// First paragraph
				const para1 = createParagraphNode();
				para1.append(createTextNode('This is a long line that needs to be wrapped properly'));
				root.append(para1);

				// Empty paragraph (paragraph break)
				const emptyPara = createParagraphNode();
				emptyPara.append(createTextNode(''));
				root.append(emptyPara);

				// Third paragraph (should not be touched)
				const para3 = createParagraphNode();
				para3.append(createTextNode('Another paragraph'));
				root.append(para3);

				// Trigger wrap on first paragraph
				const textNode = para1.getFirstChild() as LexicalTextNode;
				wrapIfNecessary({ node: textNode, maxLength: 20 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// The last paragraph should still be "Another paragraph"
				const lastPara = children[children.length - 1];
				expect(lastPara.getTextContent()).toBe('Another paragraph');
			});
		});

		it('should cascade rewrap when typing in middle causes overflow', () => {
			let textNodeKey = '';

			editor.update(() => {
				const root = getRoot();
				const paragraph = createParagraphNode();
				// Start with a line at exactly the limit
				const textNode = createTextNode('This is exactly at');
				paragraph.append(textNode);
				root.append(paragraph);
				textNodeKey = textNode.getKey();

				// Set cursor in the middle (after "This ")
				textNode.select(5, 5);
			});

			editor.update(() => {
				const node = getNodeByKey(textNodeKey) as LexicalTextNode;
				// Simulate typing " a word" in the middle, making it overflow
				node.setTextContent('This a word is exactly at');

				wrapIfNecessary({ node, maxLength: 20 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should have wrapped into 2 lines
				expect(children.length).toBeGreaterThanOrEqual(2);

				// First line should be within limit
				const firstLine = children[0].getTextContent();
				expect(firstLine.length).toBeLessThanOrEqual(20);

				// If there's a second line, it should also be within limit
				if (children.length > 1) {
					const secondLine = children[1].getTextContent();
					expect(secondLine.length).toBeLessThanOrEqual(20);
				}

				// The overflow word should have moved to the next line, not the word after cursor
				expect(firstLine).toMatch(/^This a word is$/);
				expect(children[1].getTextContent()).toBe('exactly at');
			});
		});

		it('should cascade rewrap through multiple existing paragraphs', () => {
			editor.update(() => {
				const root = getRoot();

				// Simulate previously wrapped paragraphs
				const para1 = createParagraphNode();
				para1.append(createTextNode('First line text'));
				root.append(para1);

				const para2 = createParagraphNode();
				para2.append(createTextNode('second line'));
				root.append(para2);

				const para3 = createParagraphNode();
				para3.append(createTextNode('third line'));
				root.append(para3);

				// Now edit the first paragraph to add more content
				const textNode = para1.getFirstChild() as LexicalTextNode;
				textNode.setTextContent('First line text that has been extended');

				// Trigger rewrap
				wrapIfNecessary({ node: textNode, maxLength: 20 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// All paragraphs should be within the limit
				children.forEach((child) => {
					const text = child.getTextContent();
					expect(text.length).toBeLessThanOrEqual(20);
				});

				// Should have collected and rewrapped all three original paragraphs
				// The total content should be preserved
				const allText = children.map((c) => c.getTextContent()).join(' ');
				expect(allText).toContain('First line text that has been extended');
				expect(allText).toContain('second line');
				expect(allText).toContain('third line');
			});
		});

		it('should use greedy line breaking when typing in middle of bullet list item', () => {
			let textNodeKey = '';

			editor.update(() => {
				const root = getRoot();
				const paragraph = createParagraphNode();
				// Start with a bullet at exactly the limit
				const textNode = createTextNode('- First second third');
				paragraph.append(textNode);
				root.append(paragraph);
				textNodeKey = textNode.getKey();

				// Set cursor after "First " (position 8)
				textNode.select(8, 8);
			});

			editor.update(() => {
				const node = getNodeByKey(textNodeKey) as LexicalTextNode;
				// Simulate typing "inserted " in the middle, making it overflow
				node.setTextContent('- First inserted second third');

				wrapIfNecessary({ node, maxLength: 20 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should have wrapped into at least 2 lines
				expect(children.length).toBeGreaterThanOrEqual(2);

				// First line should have the bullet and fit within maxLength
				const firstLine = children[0].getTextContent();
				expect(firstLine).toMatch(/^- /);
				expect(firstLine.length).toBeLessThanOrEqual(20);

				// The overflowing word(s) should move to the next line, not the word after cursor
				// Greedy algorithm should keep as many words as possible on first line
				expect(firstLine).toBe('- First inserted');

				// Second line should be indented and contain the overflow
				const secondLine = children[1].getTextContent();
				expect(secondLine).toMatch(/^ {2}/); // Two-space indent for bullets
				expect(secondLine.length).toBeLessThanOrEqual(20);
				expect(secondLine).toBe('  second third');
			});
		});

		it('should cascade rewrap through multiple bullet list continuation lines', () => {
			editor.update(() => {
				const root = getRoot();

				// Simulate a wrapped bullet list item
				const para1 = createParagraphNode();
				para1.append(createTextNode('- Short bullet'));
				root.append(para1);

				const para2 = createParagraphNode();
				para2.append(createTextNode('  continuation'));
				root.append(para2);

				const para3 = createParagraphNode();
				para3.append(createTextNode('  more text'));
				root.append(para3);

				// Now edit the first line to make it much longer
				const textNode = para1.getFirstChild() as LexicalTextNode;
				textNode.setTextContent('- Short bullet that has been made significantly longer');

				// Trigger rewrap
				wrapIfNecessary({ node: textNode, maxLength: 25 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// First line should start with bullet
				expect(children[0].getTextContent()).toMatch(/^- /);

				// All lines should respect maxLength
				children.forEach((child) => {
					const text = child.getTextContent();
					expect(text.length).toBeLessThanOrEqual(25);
				});

				// All continuation lines (after first) should have proper indentation
				for (let i = 1; i < children.length; i++) {
					const text = children[i].getTextContent();
					expect(text).toMatch(/^ {2}/); // Two-space indent
				}

				// All content should be preserved (strip indentation before checking)
				const allText = children
					.map((c) => c.getTextContent().replace(/^- /, '').replace(/^ {2}/, ''))
					.join(' ');
				expect(allText).toContain('Short bullet');
				expect(allText).toContain('made significantly longer');
				expect(allText).toContain('continuation');
				expect(allText).toContain('more text');
			});
		});
	});

	describe('multiple newlines at end', () => {
		it('should preserve multiple newlines at the end of text', () => {
			editor.update(() => {
				const root = getRoot();

				// Create a paragraph with text
				const para1 = createParagraphNode();
				para1.append(createTextNode('Some text'));
				root.append(para1);

				// Add empty paragraphs at the end
				const emptyPara1 = createParagraphNode();
				emptyPara1.append(createTextNode(''));
				root.append(emptyPara1);

				const emptyPara2 = createParagraphNode();
				emptyPara2.append(createTextNode(''));
				root.append(emptyPara2);

				// Trigger wrap on the first paragraph (should not affect empty paragraphs)
				const textNode = para1.getFirstChild() as LexicalTextNode;
				wrapIfNecessary({ node: textNode, maxLength: 72 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should have 3 paragraphs total
				expect(children.length).toBe(3);

				// First paragraph should have text
				expect(children[0].getTextContent()).toBe('Some text');

				// Second and third paragraphs should be empty
				expect(children[1].getTextContent()).toBe('');
				expect(children[2].getTextContent()).toBe('');
			});
		});

		it('should allow adding multiple newlines at the end by typing', () => {
			const maxLength = 72;

			// Register the IndentPlugin's Enter handler (simulating real editor behavior)
			editor.registerCommand(KEY_ENTER_COMMAND, handleEnter, COMMAND_PRIORITY_CRITICAL);

			// Register the HardWrapPlugin's mutation listener (simulating the plugin being active)
			editor.registerMutationListener(TextNode, (nodes: Map<NodeKey, NodeMutation>) => {
				editor.update(
					() => {
						for (const [key, type] of nodes.entries()) {
							if (type !== 'updated') continue;

							const node = getNodeByKey(key);
							if (!node || !isTextNode(node)) continue;

							wrapIfNecessary({ node, maxLength });
						}
					},
					{
						tag: 'history-merge'
					}
				);
			});

			// Set up initial state
			editor.update(() => {
				const root = getRoot();
				const paragraph = createParagraphNode();
				const textNode = createTextNode('Some text');
				paragraph.append(textNode);
				root.append(paragraph);
				// Set cursor at the end
				textNode.select(9, 9);
			});

			// Simulate pressing Enter (like a user would)
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			// Press Enter again
			editor.update(() => {
				editor.dispatchCommand(KEY_ENTER_COMMAND, null);
			});

			// Verify the result
			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should have 3 paragraphs total
				expect(children.length).toBe(3);

				// First paragraph should have text
				expect(children[0].getTextContent()).toBe('Some text');

				// Last two paragraphs should be empty
				expect(children[1].getTextContent()).toBe('');
				expect(children[2].getTextContent()).toBe('');
			});
		});

		it('should not remove trailing empty paragraphs when typing in the last one', () => {
			editor.update(() => {
				const root = getRoot();

				// Create initial structure with text followed by empty paragraphs
				const para1 = createParagraphNode();
				para1.append(createTextNode('Some text'));
				root.append(para1);

				const emptyPara1 = createParagraphNode();
				emptyPara1.append(createTextNode(''));
				root.append(emptyPara1);

				const emptyPara2 = createParagraphNode();
				emptyPara2.append(createTextNode(''));
				root.append(emptyPara2);

				// Now type something in the last empty paragraph
				const lastTextNode = emptyPara2.getFirstChild() as LexicalTextNode;
				lastTextNode.setTextContent('x');

				// Trigger wrapping on the last paragraph (this simulates what happens when typing)
				wrapIfNecessary({ node: lastTextNode, maxLength: 72 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should preserve all paragraphs
				expect(children.length).toBe(3);

				// Check contents
				expect(children[0].getTextContent()).toBe('Some text');
				expect(children[1].getTextContent()).toBe('');
				expect(children[2].getTextContent()).toBe('x');
			});
		});

		it('should not remove trailing empty paragraphs when typing in the first paragraph', () => {
			editor.update(() => {
				const root = getRoot();

				// Create initial structure with text followed by empty paragraphs
				const para1 = createParagraphNode();
				para1.append(createTextNode('Some text'));
				root.append(para1);

				const emptyPara1 = createParagraphNode();
				emptyPara1.append(createTextNode(''));
				root.append(emptyPara1);

				const emptyPara2 = createParagraphNode();
				emptyPara2.append(createTextNode(''));
				root.append(emptyPara2);

				// Now add more text to the first paragraph
				const firstTextNode = para1.getFirstChild() as LexicalTextNode;
				firstTextNode.setTextContent('Some text with more content added');

				// Trigger wrapping on the first paragraph (this simulates what happens when typing)
				wrapIfNecessary({ node: firstTextNode, maxLength: 72 });
			});

			editor.read(() => {
				const root = getRoot();
				const children = root.getChildren();

				// Should preserve all paragraphs - the empty ones should NOT be collected!
				expect(children.length).toBe(3);

				// Check contents
				expect(children[0].getTextContent()).toBe('Some text with more content added');
				expect(children[1].getTextContent()).toBe('');
				expect(children[2].getTextContent()).toBe('');
			});
		});
	});
});
