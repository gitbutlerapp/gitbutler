import { getHighlighter, SFC_LANGUAGES, tokenizeLine } from "$lib/utils/shikiHighlighter";
export { langFromExtension, langFromFilename } from "$lib/utils/shikiHighlighter";
import diff_match_patch from "diff-match-patch";
import type { BrandedId } from "$lib/utils/branding";

export function parseHunk(hunkStr: string): Hunk {
	const lines = hunkStr.split("\n");
	const headerLine = lines[0];
	const bodyLines = lines.slice(1);

	const hunk: Hunk = {
		...parseHeader(headerLine),
		contentSections: [],
	};

	let lastBefore = hunk.oldStart;
	let lastAfter = hunk.newStart;

	const lastLineNumberBefore = hunk.oldStart + hunk.oldLines - 1;
	const lastLineNumberAfter = hunk.newStart + hunk.newLines - 1;

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
			if (lastBefore > lastLineNumberBefore || lastAfter > lastLineNumberAfter) continue;
			lastSection.lines.push({
				afterLineNumber: lastAfter,
				beforeLineNumber: lastBefore,
				content: line.slice(1),
			});
			lastAfter += 1;
			lastBefore += 1;
		}
	}

	return hunk;
}

export type DependencyLockTarget =
	| {
			type: "stack";
			subject: string;
	  }
	| {
			type: "unidentified";
	  };

export type DependencyLock = {
	target: DependencyLockTarget;
	commitId: string;
};

export type LineLock = LineId & {
	locks: DependencyLock[];
};

/**
 * Base row without selection/lock state - this is the expensive part to compute
 * and can be cached since it only depends on the diff content.
 */
type BaseRow = {
	encodedLineId: DiffFileLineId;
	beforeLineNumber?: number;
	afterLineNumber?: number;
	tokens: string[];
	type: SectionType;
	size: number;
	isLast: boolean;
	isDeltaLine: boolean;
};

/**
 * Full row with selection and lock state applied.
 */
export type Row = BaseRow & {
	isSelected?: boolean;
	isFirstOfSelectionGroup?: boolean;
	isLastOfSelectionGroup?: boolean;
	isLastSelected?: boolean;
	locks: DependencyLock[] | undefined;
};

function getLocks(
	beforeLineNumber: number | undefined,
	afterLineNumber: number | undefined,
	lineLocks: LineLock[] | undefined,
): DependencyLock[] | undefined {
	if (!lineLocks) {
		return undefined;
	}

	const lineLock = lineLocks.find(
		(lineLock) => lineLock.oldLine === beforeLineNumber && lineLock.newLine === afterLineNumber,
	);

	return lineLock?.locks;
}

enum Operation {
	Equal = 0,
	Insert = 1,
	Delete = -1,
	Edit = 2,
}

export enum SectionType {
	AddedLines,
	RemovedLines,
	Context,
}

export enum CountColumnSide {
	Before,
	After,
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
	readonly oldLines: number;
	readonly newStart: number;
	readonly newLines: number;
	readonly comment?: string;
	readonly contentSections: ContentSection[];
};

type BaseDiffRows = { prevRows: BaseRow[]; nextRows: BaseRow[] };

const headerRegex =
	/@@ -(?<beforeStart>\d+),?(?<beforeCount>\d+)? \+(?<afterStart>\d+),?(?<afterCount>\d+)? @@(?<comment>.+)?/;
function parseHeader(header: string): {
	oldStart: number;
	newStart: number;
	oldLines: number;
	newLines: number;
	comment?: string;
} {
	const result = headerRegex.exec(header);
	if (!result?.groups) {
		throw new Error("Failed to parse diff header");
	}
	return {
		oldStart: parseInt(result.groups["beforeStart"]),
		oldLines: parseInt(result.groups["beforeCount"] ?? "1"),
		newStart: parseInt(result.groups["afterStart"]),
		newLines: parseInt(result.groups["afterCount"] ?? "1"),
		comment: result.groups["comment"],
	};
}

function lineType(line: string): SectionType {
	if (line.startsWith("+")) {
		return SectionType.AddedLines;
	} else if (line.startsWith("-")) {
		return SectionType.RemovedLines;
	} else {
		return SectionType.Context;
	}
}

export type DiffLineKey = BrandedId<"DiffLine">;
export type DiffFileKey = BrandedId<"DiffFile">;
export type DiffLineRange = BrandedId<"DiffLineRange">;
export type DiffFileLineId = BrandedId<"DiffFileLineId">;

export function createDiffLineKey(
	index: number,
	oldLine: number | undefined,
	newLine: number | undefined,
): DiffLineKey {
	return `${index}-${oldLine ?? ""}-${newLine ?? ""}` as DiffLineKey;
}

export type ParsedDiffLineKey = {
	index: number;
	oldLine: number | undefined;
	newLine: number | undefined;
};

export function readDiffLineKey(key: DiffLineKey): ParsedDiffLineKey | undefined {
	const [index, oldLine, newLine] = key.split("-");

	if (index === undefined || oldLine === undefined || newLine === undefined) {
		return undefined;
	}

	return {
		index: parseInt(index),
		oldLine: oldLine === "" ? undefined : parseInt(oldLine),
		newLine: newLine === "" ? undefined : parseInt(newLine),
	};
}

const DIFF_FILE_KEY_SEPARATOR = "%%-%%";

export function createDiffFileHunkKey(fileName: string, diffSha: string): DiffFileKey {
	return `${fileName}${DIFF_FILE_KEY_SEPARATOR}${diffSha}` as DiffFileKey;
}

export function readDiffFileHunkKey(key: DiffFileKey): [string, string] | undefined {
	const [fileName, diffSha] = key.split(DIFF_FILE_KEY_SEPARATOR);

	if (fileName === undefined || diffSha === undefined) {
		return undefined;
	}

	return [fileName, diffSha];
}

export function encodeSingleDiffLine(
	oldLine: number | undefined,
	newLine: number | undefined,
): DiffLineRange | undefined {
	if (newLine !== undefined) {
		return `R${newLine}` as DiffLineRange;
	}

	if (oldLine !== undefined) {
		return `L${oldLine}` as DiffLineRange;
	}

	return undefined;
}

export type DiffLine = {
	oldLine: number | undefined;
	newLine: number | undefined;
};

/**
 * Encode the lines selected from the diff into a string.
 *
 * This function expects to receive a continues selection of lines.
 */
export function encodeDiffLineRange(lineSelection: DiffLine[]): DiffLineRange | undefined {
	if (lineSelection.length === 0) return undefined;
	if (lineSelection.length === 1)
		return encodeSingleDiffLine(lineSelection[0].oldLine, lineSelection[0].newLine);

	const firstLine = encodeSingleDiffLine(lineSelection[0].oldLine, lineSelection[0].newLine);
	const lastLine = encodeSingleDiffLine(
		lineSelection[lineSelection.length - 1].oldLine,
		lineSelection[lineSelection.length - 1].newLine,
	);

	if (firstLine === undefined || lastLine === undefined) {
		// This should never happen unless data is corrupted
		throw new Error("Invalid line selection: " + JSON.stringify(lineSelection));
	}

	return `${firstLine}-${lastLine}` as DiffLineRange;
}

export function encodeDiffFileLine(
	fileName: string,
	oldLine: number | undefined,
	newLine: number | undefined,
): DiffFileLineId {
	const encodedLineNumber = encodeSingleDiffLine(oldLine, newLine);
	if (encodedLineNumber === undefined) {
		throw new Error("Invalid line number: " + JSON.stringify({ oldLine, newLine }));
	}

	return `${fileName}:${encodedLineNumber}` as DiffFileLineId;
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
	isLastSelected?: boolean;
};

function createBaseRowData(
	fileName: string,
	section: ContentSection,
	lang: string | undefined,
): BaseRow[] {
	return section.lines.map((line) => ({
		encodedLineId: encodeDiffFileLine(fileName, line.beforeLineNumber, line.afterLineNumber),
		beforeLineNumber: line.beforeLineNumber,
		afterLineNumber: line.afterLineNumber,
		tokens: toTokens(line.content, lang),
		type: section.sectionType,
		size: line.content.length,
		isLast: false,
		isDeltaLine: isDeltaLine(section.sectionType),
	}));
}

function sanitize(text: string): string {
	return text
		.replaceAll("&", "&amp;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll('"', "&quot;")
		.replaceAll("'", "&#39;");
}

/**
 * Simple LRU cache for memoizing toTokens results.
 * Uses a Map which maintains insertion order, allowing efficient LRU eviction.
 */
class LRUCache<V> {
	private cache = new Map<string, V>();

	constructor(private maxSize: number) {}

	get(key: string): V | undefined {
		const value = this.cache.get(key);
		if (value !== undefined) {
			// Move to end (most recently used) by re-inserting
			this.cache.delete(key);
			this.cache.set(key, value);
		}
		return value;
	}

	set(key: string, value: V): void {
		// If key exists, delete it first to update insertion order
		if (this.cache.has(key)) {
			this.cache.delete(key);
		} else if (this.cache.size >= this.maxSize) {
			// Evict oldest entry (first key in Map)
			const firstKey = this.cache.keys().next().value;
			if (firstKey !== undefined) {
				this.cache.delete(firstKey);
			}
		}
		this.cache.set(key, value);
	}

	clear(): void {
		this.cache.clear();
	}
}

/**
 * Helper to get or create a cache for a given language.
 */
function getLangCache<V>(
	cachesByLang: Map<string, LRUCache<V>>,
	noLangCache: LRUCache<V>,
	lang: string | undefined,
	cacheSize: number,
): LRUCache<V> {
	if (lang === undefined) {
		return noLangCache;
	}

	let cache = cachesByLang.get(lang);
	if (!cache) {
		cache = new LRUCache<V>(cacheSize);
		cachesByLang.set(lang, cache);
	}
	return cache;
}

// Cache keyed by language, then LRU cache by line content.
const tokenCacheByLang = new Map<string, LRUCache<string[]>>();
const tokenCacheNoLang = new LRUCache<string[]>(2000);

/**
 * Returns true if the shiki highlighter is currently loaded and ready.
 */
export function isHighlighterReady(): boolean {
	return getHighlighter() !== undefined;
}

/**
 * Clear all highlighting caches. Called when the app theme changes
 * so that tokens are re-generated with the new theme's colors.
 */
export function clearHighlightingCaches(): void {
	tokenCacheByLang.clear();
	tokenCacheNoLang.clear();
	baseRowsCacheByLang.clear();
	baseRowsCacheNoLang.clear();
}

/**
 * Count the number of distinct non-undefined colors in a token array.
 * Used to decide whether a grammar produced meaningful highlighting.
 */
function countDistinctColors(tokens: { color?: string }[]): number {
	const colors = new Set<string>();
	for (const t of tokens) {
		if (t.color) colors.add(t.color);
	}
	return colors.size;
}

function toTokens(inputLine: string, lang: string | undefined): string[] {
	// Don't cache results when highlighter isn't ready (plain text fallback)
	if (isHighlighterReady()) {
		const cache = getLangCache(tokenCacheByLang, tokenCacheNoLang, lang, 2000);
		const cached = cache.get(inputLine);
		if (cached !== undefined) {
			return cached;
		}

		let themedTokens = tokenizeLine(inputLine, lang);

		// SFC languages (Svelte, Vue) need full-file context to highlight
		// embedded JS/TS properly. Line-by-line tokenization often produces
		// poorly-colored tokens for script-block code. Compare with TypeScript
		// and pick whichever grammar produces more distinct colors.
		if (themedTokens && lang && SFC_LANGUAGES.has(lang) && inputLine.trim().length > 0) {
			const tsTokens = tokenizeLine(inputLine, "typescript");
			if (tsTokens && countDistinctColors(tsTokens) > countDistinctColors(themedTokens)) {
				themedTokens = tsTokens;
			}
		}

		if (themedTokens) {
			const tokens = themedTokens.map((t) => {
				const style = t.color ? ` style="color:${t.color}"` : "";
				return `<span data-no-drag${style}>${sanitize(t.content)}</span>`;
			});
			cache.set(inputLine, tokens);
			return tokens;
		}
	}

	// Fallback: no highlighting
	return [`<span data-no-drag>${sanitize(inputLine)}</span>`];
}

export function codeContentToTokens(content: string, lang: string | undefined): string[][] {
	const lines = content.split("\n");
	return lines.map((line) => toTokens(line, lang));
}

function computeBaseWordDiff(
	filename: string,
	prevSection: ContentSection,
	nextSection: ContentSection,
	lang: string | undefined,
): BaseDiffRows {
	const numberOfLines = nextSection.lines.length;
	const returnRows: BaseDiffRows = {
		prevRows: [],
		nextRows: [],
	};

	// Loop through every line in the section
	// We're only bothered with prev/next sections with equal # of lines changes
	for (let i = 0; i < numberOfLines; i++) {
		const oldLine = prevSection.lines[i];
		const newLine = nextSection.lines[i];
		const prevSectionRow: BaseRow = {
			encodedLineId: encodeDiffFileLine(
				filename,
				oldLine.beforeLineNumber,
				oldLine.afterLineNumber,
			),
			beforeLineNumber: oldLine.beforeLineNumber,
			afterLineNumber: oldLine.afterLineNumber,
			tokens: [],
			type: prevSection.sectionType,
			size: oldLine.content.length,
			isLast: false,
			isDeltaLine: isDeltaLine(prevSection.sectionType),
		};
		const nextSectionRow: BaseRow = {
			encodedLineId: encodeDiffFileLine(
				filename,
				newLine.beforeLineNumber,
				newLine.afterLineNumber,
			),
			beforeLineNumber: newLine.beforeLineNumber,
			afterLineNumber: newLine.afterLineNumber,
			tokens: [],
			type: nextSection.sectionType,
			size: newLine.content.length,
			isLast: false,
			isDeltaLine: isDeltaLine(nextSection.sectionType),
		};

		const diff = charDiff(oldLine.content, newLine.content);

		for (const token of diff) {
			const text = token[1];
			const type = token[0];

			if (type === Operation.Equal) {
				prevSectionRow.tokens.push(...toTokens(text, lang));
				nextSectionRow.tokens.push(...toTokens(text, lang));
			} else if (type === Operation.Insert) {
				nextSectionRow.tokens.push(
					`<span data-no-drag class="token-inserted">${sanitize(text)}</span>`,
				);
			} else if (type === Operation.Delete) {
				prevSectionRow.tokens.push(
					`<span data-no-drag class="token-deleted">${sanitize(text)}</span>`,
				);
			}
		}
		returnRows.nextRows.push(nextSectionRow);
		returnRows.prevRows.push(prevSectionRow);
	}

	return returnRows;
}

function computeBaseInlineWordDiff(
	fileName: string,
	prevSection: ContentSection,
	nextSection: ContentSection,
	lang: string | undefined,
): BaseRow[] {
	const numberOfLines = nextSection.lines.length;
	const rows: BaseRow[] = [];

	// Loop through every line in the section
	// We're only bothered with prev/next sections with equal # of lines changes
	for (let i = 0; i < numberOfLines; i++) {
		const oldLine = prevSection.lines[i];
		const newLine = nextSection.lines[i];

		const sectionRow: BaseRow = {
			encodedLineId: encodeDiffFileLine(
				fileName,
				newLine.beforeLineNumber,
				newLine.afterLineNumber,
			),
			beforeLineNumber: newLine.beforeLineNumber,
			afterLineNumber: newLine.afterLineNumber,
			tokens: [],
			type: nextSection.sectionType,
			size: newLine.content.length,
			isLast: false,
			isDeltaLine: isDeltaLine(nextSection.sectionType),
		};

		const diff = charDiff(oldLine.content, newLine.content);

		for (const token of diff) {
			const text = token[1];
			const type = token[0];

			if (type === Operation.Equal) {
				sectionRow.tokens.push(...toTokens(text, lang));
			} else if (type === Operation.Insert) {
				sectionRow.tokens.push(
					`<span data-no-drag class="token-inserted">${sanitize(text)}</span>`,
				);
			} else if (type === Operation.Delete) {
				sectionRow.tokens.push(
					`<span data-no-drag class="token-deleted token-strikethrough">${sanitize(text)}</span>`,
				);
			}
		}
		rows.push(sectionRow);
	}

	return rows;
}

export interface LineId {
	// The "before" or "removed" line number.
	oldLine: number | undefined;
	// The "after" or "added" line number.
	newLine: number | undefined;
}

export function lineIdKey(lineId: LineId): string {
	return `${lineId.oldLine}-${lineId.newLine}`;
}

export interface LineSelector extends LineId {
	/**
	 * Whether this is the first line in any selection group.
	 */
	isFirstOfGroup: boolean;
	/**
	 * Whether this is the last line in any selection group.
	 */
	isLastOfGroup: boolean;
	/**
	 * Whether is the very last line in the selection.
	 */
	isLast: boolean;
}

/**
 * Generate base rows without selection/lock state (the expensive computation).
 * This is cached to avoid re-computing when only selection changes.
 */
function generateBaseRows(
	filePath: string,
	subsections: ContentSection[],
	inlineUnifiedDiffs: boolean,
	lang: string | undefined,
): BaseRow[] {
	const rows = subsections.reduce((acc, nextSection, i) => {
		const prevSection = subsections[i - 1];

		// Filter out section for which we don't need to compute word diffs
		if (!prevSection || nextSection.sectionType === SectionType.Context) {
			acc.push(...createBaseRowData(filePath, nextSection, lang));
			return acc;
		}

		if (prevSection.sectionType === SectionType.Context) {
			acc.push(...createBaseRowData(filePath, nextSection, lang));
			return acc;
		}

		if (prevSection.lines.length !== nextSection.lines.length) {
			acc.push(...createBaseRowData(filePath, nextSection, lang));
			return acc;
		}

		if (isLineEmpty(prevSection.lines)) {
			acc.push(...createBaseRowData(filePath, nextSection, lang));
			return acc;
		}

		// Don't do word diff on super long lines
		if (
			prevSection.lines.some((line) => line.content.length > 300) ||
			nextSection.lines.some((line) => line.content.length > 300)
		) {
			acc.push(...createBaseRowData(filePath, nextSection, lang));
			return acc;
		}

		if (inlineUnifiedDiffs) {
			const rows = computeBaseInlineWordDiff(filePath, prevSection, nextSection, lang);

			acc.splice(-prevSection.lines.length);

			acc.push(...rows);
			return acc;
		} else {
			const { prevRows, nextRows } = computeBaseWordDiff(filePath, prevSection, nextSection, lang);

			// Insert returned row datastructures into the correct place
			// Find and replace previous rows with tokenized version
			prevRows.forEach((row, previousRowIndex) => {
				acc[acc.length - (prevRows.length - previousRowIndex)] = row;
			});

			acc.push(...nextRows);

			return acc;
		}
	}, [] as BaseRow[]);

	const last = rows.at(-1);
	if (last) {
		last.isLast = true;
	}

	return rows;
}

/**
 * Simple string hash function (djb2 algorithm).
 * Fast and produces reasonably distributed values for cache keys.
 */
function hashString(str: string): number {
	let hash = 5381;
	for (let i = 0; i < str.length; i++) {
		hash = (hash * 33) ^ str.charCodeAt(i);
	}
	return hash >>> 0; // Convert to unsigned 32-bit integer
}

/**
 * Create a stable cache key hash from subsections content.
 * Uses hashing to avoid storing large strings as cache keys.
 */
function hashSubsections(subsections: ContentSection[]): number {
	let hash = 5381;
	for (const section of subsections) {
		hash = (hash * 33) ^ section.sectionType;
		for (const line of section.lines) {
			hash = (hash * 33) ^ (line.beforeLineNumber ?? 0);
			hash = (hash * 33) ^ (line.afterLineNumber ?? 0);
			hash = (hash * 33) ^ hashString(line.content);
		}
	}
	return hash >>> 0;
}

// Cache for base rows: keyed by language, bounded by LRU eviction
const baseRowsCacheByLang = new Map<string, LRUCache<BaseRow[]>>();
const baseRowsCacheNoLang = new LRUCache<BaseRow[]>(100);

function getCachedBaseRows(
	filePath: string,
	subsections: ContentSection[],
	inlineUnifiedDiffs: boolean,
	lang: string | undefined,
): BaseRow[] {
	// Don't use cache when highlighter isn't ready — avoids caching un-highlighted rows
	// that would persist after shiki loads.
	if (isHighlighterReady()) {
		const cache = getLangCache(baseRowsCacheByLang, baseRowsCacheNoLang, lang, 100);
		const cacheKey = `${filePath}|${inlineUnifiedDiffs}|${hashSubsections(subsections)}`;
		const cached = cache.get(cacheKey);
		if (cached !== undefined) {
			return cached;
		}

		const baseRows = generateBaseRows(filePath, subsections, inlineUnifiedDiffs, lang);
		cache.set(cacheKey, baseRows);
		return baseRows;
	}

	return generateBaseRows(filePath, subsections, inlineUnifiedDiffs, lang);
}

/**
 * Apply selection and lock state to base rows.
 * This is a fast O(n) operation that runs on each render.
 */
function applyRowState(
	baseRows: BaseRow[],
	selectedLines: LineSelector[] | undefined,
	lineLocks: LineLock[] | undefined,
): Row[] {
	return baseRows.map((baseRow) => {
		const selectionParams = selectedLines ? getSelectionParamsForRow(baseRow, selectedLines) : {};

		return {
			...baseRow,
			locks: getLocks(baseRow.beforeLineNumber, baseRow.afterLineNumber, lineLocks),
			...selectionParams,
		};
	});
}

function getSelectionParamsForRow(row: BaseRow, selectedLines: LineSelector[]): SelectionParams {
	const selectedLine = selectedLines.find(
		(sel) => sel.oldLine === row.beforeLineNumber && sel.newLine === row.afterLineNumber,
	);

	if (!selectedLine) {
		return {};
	}

	return {
		isSelected: true,
		isFirstOfSelectionGroup: selectedLine.isFirstOfGroup,
		isLastOfSelectionGroup: selectedLine.isLastOfGroup,
		isLastSelected: selectedLine.isLast,
	};
}

export function generateRows(
	filePath: string,
	subsections: ContentSection[],
	inlineUnifiedDiffs: boolean,
	lang: string | undefined,
	selectedLines: LineSelector[] | undefined,
	lineLocks: LineLock[] | undefined,
): Row[] {
	// Get cached base rows (expensive computation)
	const baseRows = getCachedBaseRows(filePath, subsections, inlineUnifiedDiffs, lang);

	// Apply selection/lock state (cheap computation)
	return applyRowState(baseRows, selectedLines, lineLocks);
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
		afterLineCount,
	};
}

/**
 * Check if a given diff row is a line of actual changed code.
 */
export function isDeltaLine(type: SectionType): boolean {
	return [SectionType.AddedLines, SectionType.RemovedLines].includes(type);
}
