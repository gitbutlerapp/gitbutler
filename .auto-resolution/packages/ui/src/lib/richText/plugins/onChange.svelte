<script lang="ts" module>
	export type OnChangeCallback = (
		value: string,
		textUpToAnchor: string | undefined,
		textAfterAnchor: string | undefined
	) => void;
</script>

<script lang="ts">
	import { getEditor } from '$lib/richText/context';
	import { getMarkdownString } from '$lib/richText/markdown';
	import {
		getEditorTextAfterAnchor,
		getEditorTextUpToAnchor,
		setEditorText
	} from '$lib/richText/selection';
	import {
		$getRoot as getRoot,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection
	} from 'lexical';
	import { untrack } from 'svelte';

	type Props = {
		markdown: boolean;
		onChange?: OnChangeCallback;
		wrapCountValue?: number;
	};

	const { markdown, onChange, wrapCountValue }: Props = $props();

	const editor = getEditor();

	let text = $state<string>();

	function getCurrentText() {
		// If WYSIWYG is enabled, we need to transform the content to markdown strings
		if (untrack(() => markdown)) return getMarkdownString();
		return getRoot().getTextContent();
	}

	/**
	 * Splits a given word into chunks of specified length and appends a dash ('-') to all but the last chunk.
	 *
	 * @param word - The word to be split into chunks.
	 * @param wrap - The maximum length of each chunk (the actual chunk size will be wrap - 1 to leave space for the dash).
	 * @returns An array of string chunks, with dashes appended to all but the last chunk.
	 * @throws Error if the word cannot be split into chunks.
	 */
	function wrapWord(word: string, wrap: number): string[] {
		// Split the word into chunks of length wrap - 1
		// to leave space for the dash.
		const chunks = word.match(new RegExp(`.{1,${wrap - 1}}`, 'g'));
		if (chunks) {
			// Append a dash to all but the last chunk.
			for (let i = 0; i < chunks.length - 1; i++) {
				chunks[i] += '-';
			}
			return chunks;
		}
		throw new Error('Failed to wrap word');
	}

	/**
	 * Wraps the text to a given length
	 *
	 * Doesn't break words, but will break lines unless the word is longer than the wrap length.
	 */
	function wrapText(text: string, wrap: number): string {
		const lines = text.split('\n');
		let buffer: string[] = [];
		for (const line of lines) {
			if (line.length > wrap) {
				const words = line.split(' ');
				let currentLine = '';
				for (const word of words) {
					if (word.length >= wrap) {
						if (currentLine.length > 0) {
							buffer.push(currentLine);
							currentLine = '';
						}
						// If the word is longer than the wrap, we need to break it
						const chunks = wrapWord(word, wrap);
						buffer.push(...chunks);
						continue;
					}

					// If the sum of the current line, a space, and the word is greater than the wrap,
					// we need to break the line.
					if (currentLine.length + word.length + 1 > wrap) {
						buffer.push(currentLine);
						currentLine = '';
					}
					if (currentLine.length > 0) {
						currentLine += ' ';
					}
					currentLine += word;
				}

				if (currentLine.length > 0) {
					buffer.push(currentLine);
				}
				continue;
			}
			buffer.push(line);
		}

		return buffer.join('\n');
	}

	$effect(() => {
		return editor.registerUpdateListener(
			({ editorState, dirtyElements, dirtyLeaves, prevEditorState, tags }) => {
				if (
					tags.has('history-merge') ||
					(dirtyElements.size === 0 && dirtyLeaves.size === 0) ||
					prevEditorState.isEmpty()
				) {
					return;
				}

				editorState.read(() => {
					const currentText = getCurrentText();
					if (currentText === text) {
						return;
					}

					text = currentText;
					const selection = getSelection();
					if (!isRangeSelection(selection)) {
						return;
					}

					const textUpToAnchor = getEditorTextUpToAnchor(selection);
					const textAfterAnchor = getEditorTextAfterAnchor(selection);
					onChange?.(text, textUpToAnchor, textAfterAnchor);
				});
			}
		);
	});

	$effect(() => {
		if (!markdown && wrapCountValue && text) {
			const wrappedText = wrapText(text, wrapCountValue);
			if (wrappedText === text) {
				return;
			}
			setEditorText(editor, wrappedText);
			return;
		}
	});

	export function getText(): string | undefined {
		return text;
	}
</script>
