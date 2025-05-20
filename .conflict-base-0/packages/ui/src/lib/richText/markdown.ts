import { isWrappingExempt, parseBullet, parseIndent, wrapLine } from '$lib/richText/linewrap';
import {
	$convertFromMarkdownString as convertFromMarkdownString,
	$convertToMarkdownString as convertToMarkdownString
} from '@lexical/markdown';
import {
	$getRoot as getRoot,
	type LexicalEditor,
	$createParagraphNode as createParagraphNode,
	$createTextNode as createTextNode
} from 'lexical';
import { ALL_TRANSFORMERS } from 'svelte-lexical';

export function updateEditorToMarkdown(editor: LexicalEditor | undefined) {
	editor?.update(() => {
		const text = getRoot().getTextContent();
		convertFromMarkdownString(text, ALL_TRANSFORMERS, undefined, false, true);
	});
}

/**
 * TODO: We should not call this on _every_ change to the document, see `OnChange.svelte`.
 */
export function getMarkdownString(maxLength?: number): string {
	const markdown = convertToMarkdownString(ALL_TRANSFORMERS);
	return maxLength ? wrapIfNecessary(markdown, maxLength) : markdown;
}

/**
 * Gets the number of lines, starting from the current one, that belong to the
 * same paragraph.
 */
function getParagraphLength(lines: string[], indent: string): number {
	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];
		if (
			line.trimStart() === '' &&
			!line.startsWith(indent) &&
			line.length > indent.length &&
			line[indent.length].match(/\s/)
		) {
			return i;
		}
	}
	return lines.length - 1;
}

/**
 * Takes output from the lexical rich text -> markdown conversion, and hard
 * wraps the output according to the given maxLength.
 */
function wrapIfNecessary(markdown: string, maxLength: number): string {
	const lines = markdown.split('\n');
	let i = 0;
	const newLines: string[] = [];
	while (i < lines.length) {
		const line = lines[i];
		const indent = parseIndent(line);
		const bullet = parseBullet(line);

		if (line.length <= maxLength || line.indexOf(' ') === -1 || isWrappingExempt(line)) {
			newLines.push(line);
			i++;
			continue; // Line does not exceed max length.
		}

		// Remainder string that should be carried over between lines when
		// re-wrapping lines.
		let remainder = '';

		// We want to consider the modified line, and the remaining lines from
		// the same pagraph.
		const paragraphLength = getParagraphLength(lines.slice(i), indent);

		const { newLine, newRemainder } = wrapLine({
			line,
			remainder,
			maxLength,
			indent: bullet?.indent || indent,
			bullet
		});

		newLines.push(newLine);

		remainder = newRemainder;

		// Carry over possible remainder and re-wrap the rest of paragraph.
		for (let j = 0; j < paragraphLength; j++) {
			const line = lines[i + j];
			const { newLine, newRemainder } = wrapLine({ line, remainder, maxLength, indent, bullet });
			newLines.push(newLine);
			remainder = newRemainder;
		}

		// Move pointer along
		i += paragraphLength;

		// Insert any final remainder at the end of the paragraph.
		if (remainder) {
			while (remainder.length > 0) {
				const { newLine, newRemainder } = wrapLine({ line: remainder, maxLength, indent, bullet });
				newLines.push(newLine);
				remainder = newRemainder;
			}
		}
		i++;
	}
	return newLines.join('\n');
}

export function updateEditorToPlaintext(editor: LexicalEditor | undefined, maxLength?: number) {
	editor?.update(() => {
		const text = getMarkdownString(maxLength);
		const root = getRoot();
		root.clear();
		const paragraph = createParagraphNode();
		paragraph.append(createTextNode(text));
		root.append(paragraph);
	});
}
