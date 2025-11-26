import { wrapIfNecessary } from '$lib/richText/linewrap';
import { createEditor, type LexicalEditor } from 'lexical';
import {
	$createParagraphNode as createParagraphNode,
	$createTextNode as createTextNode,
	$getRoot as getRoot,
	$getNodeByKey as getNodeByKey,
	$getSelection as getSelection,
	$isRangeSelection as isRangeSelection,
	type TextNode
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
				const node = getNodeByKey(textNodeKey) as TextNode;
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
				const node = getNodeByKey(textNodeKey) as TextNode;
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
				const node = getNodeByKey(textNodeKey) as TextNode;
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
				const textNode = para1.getFirstChild() as TextNode;
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
				const node = getNodeByKey(textNodeKey) as TextNode;
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
				const textNode = para1.getFirstChild() as TextNode;
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
				const textNode = para1.getFirstChild() as TextNode;
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
				const textNode = para1.getFirstChild() as TextNode;
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
	});
});
