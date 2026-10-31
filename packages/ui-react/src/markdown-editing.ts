/**
 * Markdown formatting commands for a plain textarea.
 *
 * Every command is a pure rewrite of a textarea's value plus its selection,
 * so the toolbar can stay a dumb dispatcher and the behaviour stays testable
 * without a DOM. Bodies remain markdown source — the same text the PR view
 * renders back through {@link Markdown} — so nothing here needs a serializer.
 */

/** A textarea's content together with its selection range. */
export type MarkdownSelection = {
	readonly text: string;
	readonly start: number;
	readonly end: number;
};

export type MarkdownCommand = (selection: MarkdownSelection) => MarkdownSelection;

/** The list markers a list command replaces when switching list kinds. */
const LIST_PREFIX = /^(?:[-*+] \[[ xX]\] |[-*+] |\d+\. )/;
const HEADING_PREFIX = /^#{1,6} +/;
const QUOTE_PREFIX = /^> ?/;

/** Grow a selection to cover whole lines, which block commands rewrite. */
const lineRange = ({ text, start, end }: MarkdownSelection): { from: number; to: number } => {
	const from = start === 0 ? 0 : text.lastIndexOf("\n", start - 1) + 1;
	const lineEnd = text.indexOf("\n", end);
	return { from, to: lineEnd === -1 ? text.length : lineEnd };
};

/**
 * Rewrite each line the selection touches, leaving the whole block selected —
 * the caret has no single sensible home once line lengths change.
 */
const mapLines =
	(transform: (line: string, index: number) => string): MarkdownCommand =>
	(selection) => {
		const { from, to } = lineRange(selection);
		const rewritten = selection.text.slice(from, to).split("\n").map(transform).join("\n");
		return {
			text: selection.text.slice(0, from) + rewritten + selection.text.slice(to),
			start: from,
			end: from + rewritten.length,
		};
	};

/**
 * Toggle a line prefix across the selection. `prefix` takes the line's index
 * so ordered lists can number themselves; `replaces` is the family of markers
 * the prefix supersedes (e.g. a bullet replacing a task box).
 */
const linePrefix =
	(prefix: (lineIndex: number) => string, replaces: RegExp): MarkdownCommand =>
	(selection) => {
		const { from, to } = lineRange(selection);
		const lines = selection.text.slice(from, to).split("\n");
		// Compare the marker a line already carries, not just its opening
		// characters: `- ` prefixes `- [ ] `, so a startsWith check would read a
		// task line as an applied bullet and strip two characters off its box.
		const applied = lines.every(
			(line, index) => (replaces.exec(line)?.[0] ?? "") === prefix(index),
		);
		return mapLines((line, index) =>
			applied ? line.slice(prefix(index).length) : prefix(index) + line.replace(replaces, ""),
		)(selection);
	};

/** Strip a family of line markers without adding one back. */
const stripLinePrefix = (replaces: RegExp): MarkdownCommand =>
	mapLines((line) => line.replace(replaces, ""));

/**
 * Toggle an inline marker around the selection. Unwraps when the markers sit
 * either just inside the selection (the user re-selected what they wrapped)
 * or just outside it (they selected only the text between them).
 */
const wrap =
	(marker: string): MarkdownCommand =>
	({ text, start, end }) => {
		const selected = text.slice(start, end);

		if (
			selected.length >= marker.length * 2 &&
			selected.startsWith(marker) &&
			selected.endsWith(marker)
		) {
			const inner = selected.slice(marker.length, -marker.length);
			return {
				text: text.slice(0, start) + inner + text.slice(end),
				start,
				end: start + inner.length,
			};
		}

		if (
			start >= marker.length &&
			text.slice(start - marker.length, start) === marker &&
			text.slice(end, end + marker.length) === marker
		) {
			return {
				text: text.slice(0, start - marker.length) + selected + text.slice(end + marker.length),
				start: start - marker.length,
				end: end - marker.length,
			};
		}

		return {
			text: text.slice(0, start) + marker + selected + marker + text.slice(end),
			start: start + marker.length,
			end: end + marker.length,
		};
	};

/** Wrap the selection as a link label and leave the placeholder URL selected. */
export const link: MarkdownCommand = ({ text, start, end }) => {
	const selected = text.slice(start, end);
	const urlStart = start + selected.length + 3;
	return {
		text: `${text.slice(0, start)}[${selected}](url)${text.slice(end)}`,
		start: urlStart,
		end: urlStart + 3,
	};
};

/**
 * Drop literal markdown at the caret, replacing any selection, and leave the
 * caret after it. Unlike the other commands this takes its text from the
 * caller, because what gets inserted (an uploaded file's link) is not
 * derivable from the selection.
 */
export const insert =
	(snippet: string): MarkdownCommand =>
	({ text, start, end }) => {
		const after = start + snippet.length;
		return {
			text: `${text.slice(0, start)}${snippet}${text.slice(end)}`,
			start: after,
			end: after,
		};
	};

export const bulletList = linePrefix(() => "- ", LIST_PREFIX);
export const numberList = linePrefix((index) => `${index + 1}. `, LIST_PREFIX);
export const taskList = linePrefix(() => "- [ ] ", LIST_PREFIX);
export const quote = linePrefix(() => "> ", QUOTE_PREFIX);
export const heading2 = linePrefix(() => "## ", HEADING_PREFIX);
export const heading3 = linePrefix(() => "### ", HEADING_PREFIX);
export const plainText = stripLinePrefix(HEADING_PREFIX);
export const bold = wrap("**");
export const italic = wrap("_");
export const strikethrough = wrap("~~");
export const code = wrap("`");
