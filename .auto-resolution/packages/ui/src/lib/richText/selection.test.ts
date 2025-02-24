import { setEditorText } from '$lib/richText/selection';
import { createEditor, type LexicalEditor } from 'lexical';
import { $getRoot, $isParagraphNode } from 'lexical';
import { describe, it, expect, beforeEach } from 'vitest';

describe('setEditorText', () => {
	let editor: LexicalEditor;

	beforeEach(() => {
		editor = createEditor({
			namespace: 'test',
			onError: (error) => {
				throw error;
			}
		});
	});

	it('should preserve blank lines when setting text', () => {
		const textWithBlankLines = `First paragraph

Second paragraph

Third paragraph`;

		setEditorText(editor, textWithBlankLines);

		editor.read(() => {
			const root = $getRoot();
			const children = root.getChildren();

			// Should have 5 paragraphs: para1, blank, para2, blank, para3
			expect(children.length).toBe(5);

			// Check that paragraphs 2 and 4 are empty (blank lines)
			const para2 = children[1];
			const para4 = children[3];

			if ($isParagraphNode(para2)) {
				expect(para2.getTextContent()).toBe('');
			}

			if ($isParagraphNode(para4)) {
				expect(para4.getTextContent()).toBe('');
			}
		});
	});

	it('should handle multiple consecutive blank lines', () => {
		const textWithMultipleBlankLines = `First


Third`;

		setEditorText(editor, textWithMultipleBlankLines);

		editor.read(() => {
			const root = $getRoot();
			const children = root.getChildren();

			// Should have 4 paragraphs: para1, blank, blank, para3
			expect(children.length).toBe(4);
		});
	});
});
