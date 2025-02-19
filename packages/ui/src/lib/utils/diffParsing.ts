import { cpp } from '@codemirror/lang-cpp';
import { css } from '@codemirror/lang-css';
import { html } from '@codemirror/lang-html';
import { java } from '@codemirror/lang-java';
import { javascript } from '@codemirror/lang-javascript';
import { json } from '@codemirror/lang-json';
import { markdown } from '@codemirror/lang-markdown';
import { php } from '@codemirror/lang-php';
import { python } from '@codemirror/lang-python';
// import { svelte } from '@replit/codemirror-lang-svelte';
import { rust } from '@codemirror/lang-rust';
import { vue } from '@codemirror/lang-vue';
import { wast } from '@codemirror/lang-wast';
import { xml } from '@codemirror/lang-xml';
import { HighlightStyle, StreamLanguage } from '@codemirror/language';
import { ruby } from '@codemirror/legacy-modes/mode/ruby';
import { NodeType, Tree, Parser } from '@lezer/common';
import { tags, highlightTree } from '@lezer/highlight';
import diff_match_patch from 'diff-match-patch';

export function parseHunk(hunkStr: string): Hunk {
	const lines = hunkStr.trim().split('\n');
	const headerLine = lines[0];
	const bodyLines = lines.slice(1);

	const hunk: Hunk = {
		...parseHeader(headerLine),
		contentSections: []
	};

	let lastBefore = hunk.oldStart;
	let lastAfter = hunk.newStart;

	for (const line of bodyLines) {
		const type = lineType(line);
		let lastSection = hunk.contentSections.at(-1);
		if (!lastSection) {
			lastSection = { lines: [], sectionType: type };
			hunk.contentSections.push(lastSection);
		}

		// If the type has changed, we want to start a new section
		if (lastSection.sectionType !== type) {
			lastSection = { lines: [], sectionType: type };
			hunk.contentSections.push(lastSection);
		}

		if (type === SectionType.AddedLines) {
			lastSection.lines.push({ afterLineNumber: lastAfter, content: line.slice(1) });
			lastAfter += 1;
		} else if (type === SectionType.RemovedLines) {
			lastSection.lines.push({ beforeLineNumber: lastBefore, content: line.slice(1) });
			lastBefore += 1;
		} else {
			lastSection.lines.push({
				afterLineNumber: lastAfter,
				beforeLineNumber: lastBefore,
				content: line.slice(1)
			});
			lastAfter += 1;
			lastBefore += 1;
		}
	}

	return hunk;
}

export type Row = {
	beforeLineNumber?: number;
	afterLineNumber?: number;
	tokens: string[];
	type: SectionType;
	size: number;
	isLast: boolean;
	isSelected?: boolean;
	isFirstOfSelectionGroup?: boolean;
	isLastOfSelectionGroup?: boolean;
};

enum Operation {
	Equal = 0,
	Insert = 1,
	Delete = -1,
	Edit = 2
}

export enum SectionType {
	AddedLines,
	RemovedLines,
	Context
}

export enum CountColumnSide {
	Before,
	After
}

export type Line = {
	readonly beforeLineNumber?: number;
	readonly afterLineNumber?: number;
	readonly content: string;
};

export type ContentSection = {
	readonly lines: Line[];
	readonly sectionType: SectionType;
};

type Hunk = {
	readonly oldStart: number;
	readonly newStart: number;
	readonly contentSections: ContentSection[];
};

type DiffRows = { prevRows: Row[]; nextRows: Row[] };

const headerRegex =
	/@@ -(?<beforeStart>\d+),?(?<beforeCount>\d+)? \+(?<afterStart>\d+),?(?<afterCount>\d+)? @@(?<comment>.+)?/;
function parseHeader(header: string): { oldStart: number; newStart: number } {
	const result = headerRegex.exec(header);
	if (!result?.groups) {
		throw new Error('Failed to parse diff header');
	}
	return {
		oldStart: parseInt(result.groups['beforeStart']),
		newStart: parseInt(result.groups['afterStart'])
	};
}

function lineType(line: string): SectionType {
	if (line.startsWith('+')) {
		return SectionType.AddedLines;
	} else if (line.startsWith('-')) {
		return SectionType.RemovedLines;
	} else {
		return SectionType.Context;
	}
}

const t = tags;

const highlightStyle: HighlightStyle = HighlightStyle.define([
	{ tag: t.variableName, class: 'token-variable' },
	{ tag: t.definition(t.variableName), class: 'token-definition' },
	{ tag: t.propertyName, class: 'token-property' },
	{ tag: [t.typeName, t.className, t.namespace, t.macroName], class: 'token-type' },
	{ tag: [t.special(t.name), t.constant(t.className)], class: 'token-variable-special' },
	{ tag: t.standard(t.variableName), class: 'token-builtin' },

	{ tag: [t.number, t.literal, t.unit], class: 'token-number' },
	{ tag: t.string, class: 'token-string' },
	{ tag: [t.special(t.string), t.regexp, t.escape], class: 'token-string-special' },
	{ tag: [], class: 'token-atom' },

	{ tag: t.keyword, class: 'token-keyword' },
	{ tag: [t.comment, t.quote], class: 'token-comment' },
	{ tag: t.meta, class: 'token-meta' },
	{ tag: t.invalid, class: 'token-invalid' },

	{ tag: t.tagName, class: 'token-tag' },
	{ tag: t.attributeName, class: 'token-attribute' },
	{ tag: t.attributeValue, class: 'token-attribute-value' },

	{ tag: t.inserted, class: 'token-inserted' },
	{ tag: t.deleted, class: 'token-deleted' },
	{ tag: t.heading, class: 'token-heading' },
	{ tag: t.link, class: 'token-link' },
	{ tag: t.strikethrough, class: 'token-strikethrough' },
	{ tag: t.strong, class: 'token-strong' },
	{ tag: t.emphasis, class: 'token-emphasis' }
]);

function create(code: string, parser: Parser | undefined): CodeHighlighter {
	let tree: Tree;
	if (parser) {
		tree = parser.parse(code);
	} else {
		tree = new Tree(NodeType.none, [], [], code.length);
	}
	return new CodeHighlighter(code, tree);
}

export function parserFromFilename(filename: string): Parser | undefined {
	const ext = filename.split('.').pop();
	switch (ext) {
		case 'jsx':
		case 'js':
			// We intentionally allow JSX in normal .js as well as .jsx files,
			// because there are simply too many existing applications and
			// examples out there that use JSX within .js files, and we don't
			// want to break them.
			return javascript({ jsx: true }).language.parser;
		case 'ts':
			return javascript({ typescript: true }).language.parser;
		case 'tsx':
			return javascript({ typescript: true, jsx: true }).language.parser;

		case 'css':
			return css().language.parser;

		case 'html':
			return html({ selfClosingTags: true }).language.parser;

		case 'xml':
			return xml().language.parser;

		case 'wasm':
			return wast().language.parser;

		case 'cpp':
		case 'c++':
		case 'hpp':
		case 'h++':
			return cpp().language.parser;

		// case 'text/x-go':
		//     return new LanguageSupport(await CodeMirror.go());

		case 'java':
			return java().language.parser;

		// case 'text/x-kotlin':
		//     return new LanguageSupport(await CodeMirror.kotlin());

		case 'json':
			return json().language.parser;

		case 'php':
			return php().language.parser;

		case 'py':
		case 'python':
			return python().language.parser;

		case 'md':
			return markdown().language.parser;

		// case 'text/x-sh':
		//     return new LanguageSupport(await CodeMirror.shell());

		// case 'text/x-coffeescript':
		//     return new LanguageSupport(await CodeMirror.coffeescript());

		// case 'text/x-clojure':
		//     return new LanguageSupport(await CodeMirror.clojure());

		// case 'application/vnd.dart':
		//     return new LanguageSupport(await CodeMirror.dart());

		// case 'text/x-gss':
		//     return new LanguageSupport(await CodeMirror.gss());

		// case 'text/x-less':
		//     return new CodeMirror.LanguageSupport(await CodeMirror.less());

		// case 'text/x-sass':
		//     return new LanguageSupport(await CodeMirror.sass());

		// case 'text/x-scala':
		//     return new LanguageSupport(await CodeMirror.scala());

		// case 'text/x-scss':
		//     return new LanguageSupport(await CodeMirror.scss());

		case 'svelte':
			// TODO: is codemirror-lang-svelte broken or just not used correctly?
			// return svelte();

			// highlighting svelte with js + jsx works much better than the above
			return javascript({ typescript: true, jsx: true }).language.parser;

		case 'vue':
			return vue().language.parser;

		case 'rs':
			return rust().language.parser;

		case 'rb':
			return StreamLanguage.define(ruby).parser;

		default:
			return undefined;
	}
}

class CodeHighlighter {
	constructor(
		readonly code: string,
		readonly tree: Tree
	) {}

	highlight(token: (text: string, style: string) => void): void {
		this.highlightRange(0, this.code.length, token);
	}

	highlightRange(from: number, to: number, token: (text: string, style: string) => void): void {
		let pos = from;
		const flush = (to: number, style: string): void => {
			if (to > pos) {
				token(this.code.slice(pos, to), style);
				pos = to;
			}
		};
		highlightTree(
			this.tree,
			highlightStyle,
			(from, to, style) => {
				flush(from, '');
				flush(to, style);
			},
			from,
			to
		);
		flush(to, '');
	}
}

function charDiff(text1: string, text2: string): { 0: number; 1: string }[] {
	const differ = new diff_match_patch();
	const diff = differ.diff_main(text1, text2);
	differ.diff_cleanupSemantic(diff);
	return diff;
}

const WHITESPACE_REGEX = new RegExp(/\s/);

function isLineEmpty(lines: Line[]) {
	if (!lines[0]?.content.match(WHITESPACE_REGEX)) {
		return true;
	}

	return false;
}

type SelectionParams = {
	isSelected?: boolean;
	isFirstOfSelectionGroup?: boolean;
	isLastOfSelectionGroup?: boolean;
};

function getSelectionParams(
	line: Line,
	selectedLines: LineSelector[] | undefined
): SelectionParams {
	if (!selectedLines) {
		return {};
	}

	const selectedLine = selectedLines.find(
		(selectedLine) =>
			selectedLine.oldLine === line.beforeLineNumber &&
			selectedLine.newLine === line.afterLineNumber
	);

	if (!selectedLine) {
		return {};
	}

	return {
		isSelected: true,
		isFirstOfSelectionGroup: selectedLine.isFirstOfGroup,
		isLastOfSelectionGroup: selectedLine.isLastOfGroup
	};
}

function createRowData(
	section: ContentSection,
	parser: Parser | undefined,
	selectedLines: LineSelector[] | undefined
): Row[] {
	return section.lines.map((line) => {
		// if (line.content === '') {
		// 	// Add extra \n for empty lines for correct copy/pasting output
		// 	line.content = '\n';
		// }

		return {
			beforeLineNumber: line.beforeLineNumber,
			afterLineNumber: line.afterLineNumber,
			tokens: toTokens(line.content, parser),
			type: section.sectionType,
			size: line.content.length,
			isLast: false,
			...getSelectionParams(line, selectedLines)
		};
	});
}

function sanitize(text: string) {
	const element = document.createElement('div');
	element.innerText = text;
	return element.innerHTML;
}

function toTokens(inputLine: string, parser: Parser | undefined): string[] {
	const highlighter = create(inputLine, parser);
	const tokens: string[] = [];
	highlighter.highlight((text, classNames) => {
		const token = classNames
			? `<span data-no-drag class=${classNames}>${sanitize(text)}</span>`
			: sanitize(text);

		tokens.push(token);
	});

	return tokens;
}

function computeWordDiff(
	prevSection: ContentSection,
	nextSection: ContentSection,
	parser: Parser | undefined,
	selectedLines: LineSelector[] | undefined
): DiffRows {
	const numberOfLines = nextSection.lines.length;
	const returnRows: DiffRows = {
		prevRows: [],
		nextRows: []
	};

	// Loop through every line in the section
	// We're only bothered with prev/next sections with equal # of lines changes
	for (let i = 0; i < numberOfLines; i++) {
		const oldLine = prevSection.lines[i] as Line;
		const newLine = nextSection.lines[i] as Line;
		const prevSectionRow = {
			beforeLineNumber: oldLine.beforeLineNumber,
			afterLineNumber: oldLine.afterLineNumber,
			tokens: [] as string[],
			type: prevSection.sectionType,
			size: oldLine.content.length,
			isLast: false,
			...getSelectionParams(oldLine, selectedLines)
		};
		const nextSectionRow = {
			beforeLineNumber: newLine.beforeLineNumber,
			afterLineNumber: newLine.afterLineNumber,
			tokens: [] as string[],
			type: nextSection.sectionType,
			size: newLine.content.length,
			isLast: false,
			...getSelectionParams(newLine, selectedLines)
		};

		const diff = charDiff(oldLine.content, newLine.content);

		for (const token of diff) {
			const text = token[1];
			const type = token[0];

			if (type === Operation.Equal) {
				prevSectionRow.tokens.push(...toTokens(text, parser));
				nextSectionRow.tokens.push(...toTokens(text, parser));
			} else if (type === Operation.Insert) {
				nextSectionRow.tokens.push(
					`<span data-no-drag class="token-inserted">${sanitize(text)}</span>`
				);
			} else if (type === Operation.Delete) {
				prevSectionRow.tokens.push(
					`<span data-no-drag class="token-deleted">${sanitize(text)}</span>`
				);
			}
		}
		returnRows.nextRows.push(nextSectionRow);
		returnRows.prevRows.push(prevSectionRow);
	}

	return returnRows;
}

function computeInlineWordDiff(
	prevSection: ContentSection,
	nextSection: ContentSection,
	parser: Parser | undefined,
	selectedLines: LineSelector[] | undefined
): Row[] {
	const numberOfLines = nextSection.lines.length;

	const rows = [];

	// Loop through every line in the section
	// We're only bothered with prev/next sections with equal # of lines changes
	for (let i = 0; i < numberOfLines; i++) {
		const oldLine = prevSection.lines[i] as Line;
		const newLine = nextSection.lines[i] as Line;

		const sectionRow = {
			beforeLineNumber: newLine.beforeLineNumber,
			afterLineNumber: newLine.afterLineNumber,
			tokens: [] as string[],
			type: nextSection.sectionType,
			size: newLine.content.length,
			isLast: false,
			...getSelectionParams(newLine, selectedLines)
		};

		const diff = charDiff(oldLine.content, newLine.content);

		for (const token of diff) {
			const text = token[1];
			const type = token[0];

			if (type === Operation.Equal) {
				sectionRow.tokens.push(...toTokens(text, parser));
			} else if (type === Operation.Insert) {
				sectionRow.tokens.push(
					`<span data-no-drag class="token-inserted">${sanitize(text)}</span>`
				);
			} else if (type === Operation.Delete) {
				sectionRow.tokens.push(
					`<span data-no-drag class="token-deleted token-strikethrough">${sanitize(text)}</span>`
				);
			}
		}
		rows.push(sectionRow);
	}

	return rows;
}

export interface LineSelector {
	oldLine: number | undefined;
	newLine: number | undefined;
	/**
	 * Whether this is the first line in any selection group.
	 */
	isFirstOfGroup: boolean;
	/**
	 * Whether this is the last line in any selection group.
	 */
	isLastOfGroup: boolean;
}

export function generateRows(
	subsections: ContentSection[],
	inlineUnifiedDiffs: boolean,
	parser: Parser | undefined,
	selectedLines: LineSelector[] | undefined
) {
	const rows = subsections.reduce((acc, nextSection, i) => {
		const prevSection = subsections[i - 1];

		// Filter out section for which we don't need to compute word diffs
		if (!prevSection || nextSection.sectionType === SectionType.Context) {
			acc.push(...createRowData(nextSection, parser, selectedLines));
			return acc;
		}

		if (prevSection.sectionType === SectionType.Context) {
			acc.push(...createRowData(nextSection, parser, selectedLines));
			return acc;
		}

		if (prevSection.lines.length !== nextSection.lines.length) {
			acc.push(...createRowData(nextSection, parser, selectedLines));
			return acc;
		}

		if (isLineEmpty(prevSection.lines)) {
			acc.push(...createRowData(nextSection, parser, selectedLines));
			return acc;
		}

		if (inlineUnifiedDiffs) {
			const rows = computeInlineWordDiff(prevSection, nextSection, parser, selectedLines);

			acc.splice(-prevSection.lines.length);

			acc.push(...rows);
			return acc;
		} else {
			const { prevRows, nextRows } = computeWordDiff(
				prevSection,
				nextSection,
				parser,
				selectedLines
			);

			// Insert returned row datastructures into the correct place
			// Find and replace previous rows with tokenized version
			prevRows.forEach((row, previousRowIndex) => {
				acc[acc.length - (prevRows.length - previousRowIndex)] = row;
			});

			acc.push(...nextRows);

			return acc;
		}
	}, [] as Row[]);

	const last = rows.at(-1);
	if (last) {
		last.isLast = true;
	}

	return rows;
}

interface DiffHunkLineInfo {
	beforLineStart: number;
	beforeLineCount: number;
	afterLineStart: number;
	afterLineCount: number;
}

export function getHunkLineInfo(subsections: ContentSection[]): DiffHunkLineInfo {
	const firstSection = subsections[0];
	const lastSection = subsections.at(-1);

	const beforLineStart = firstSection?.lines[0]?.beforeLineNumber ?? 0;
	const beforeLineEnd = lastSection?.lines?.at(-1)?.beforeLineNumber ?? 0;
	const beforeLineCount = beforeLineEnd - beforLineStart + 1;

	const afterLineStart = firstSection?.lines[0]?.afterLineNumber ?? 0;
	const afterLineEnd = lastSection?.lines?.at(-1)?.afterLineNumber ?? 0;
	const afterLineCount = afterLineEnd - afterLineStart + 1;

	return {
		beforLineStart,
		beforeLineCount,
		afterLineStart,
		afterLineCount
	};
}
