import { copyToClipboard } from '@gitbutler/shared/clipboard';
import {
	readDiffLineKey,
	type DiffLineKey,
	type DiffFileKey,
	createDiffFileHunkKey,
	createDiffLineKey,
	readDiffFileHunkKey,
	encodeDiffFileLine
} from '@gitbutler/ui/utils/diffParsing';
import { SvelteSet } from 'svelte/reactivity';
import type { ChatMinimize } from '$lib/chat/minimize.svelte';
import type { DiffPatch } from '@gitbutler/shared/chat/types';
import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
import type { ContentSection, DiffFileLineId, LineSelector } from '@gitbutler/ui/utils/diffParsing';

export interface DiffLineSelected extends LineSelector {
	index: number;
}

export interface DiffSelection {
	diffSha: string;
	fileName: string;
	lines: DiffLineSelected[];
}

/**
 * Create a diff line selection string out of a diff patch array.
 *
 * @note - This function assumes that the diff patch array is an ordered & continues selection of lines.
 */
export function parseDiffPatchToEncodedSelection(
	fileName: string,
	diffPatchArray: DiffPatch[]
): DiffFileLineId | undefined {
	if (diffPatchArray.length === 0) return undefined;
	return encodeDiffFileLine(fileName, diffPatchArray[0].left, diffPatchArray[0].right);
}

function calculateSelectedLines(selectedDiffLines: SvelteSet<DiffLineKey>): DiffLineSelected[] {
	const parsedLines = Array.from(selectedDiffLines)
		.map((key) => readDiffLineKey(key))
		.filter((l): l is DiffLineSelected => !!l);

	if (parsedLines.length === 0) return [];
	if (parsedLines.length === 1)
		return [
			{
				...parsedLines[0],
				isFirstOfGroup: true,
				isLastOfGroup: true,
				isLast: true
			}
		];

	const sortedLines = parsedLines.sort((a, b) => a.index - b.index);
	const result: DiffLineSelected[] = [];

	for (let i = 0; i < sortedLines.length; i++) {
		const current = sortedLines[i];
		const prev = sortedLines[i - 1];
		const next = sortedLines[i + 1];

		const isFirstOfGroup = !prev || current.index - prev.index > 1;
		const isLastOfGroup = !next || next.index - current.index > 1;
		const isLast = i === sortedLines.length - 1;

		result.push({
			...current,
			isFirstOfGroup,
			isLastOfGroup,
			isLast
		});
	}
	return result;
}

export default class DiffLineSelection {
	private startIndex: number | undefined;
	private _quote = $state<boolean>(false);
	private _selectedDiffLines = new SvelteSet<DiffLineKey>();
	private _selectedLines: DiffLineSelected[] = $derived(
		calculateSelectedLines(this._selectedDiffLines)
	);
	private _selectedDiffFile = $state<DiffFileKey>();

	constructor(private readonly chatMinimizer: ChatMinimize) {}

	clear(fileName?: string) {
		if (fileName && this._selectedDiffFile) {
			const parsed = readDiffFileHunkKey(this._selectedDiffFile);
			if (!parsed) return; // This should never happen

			const [selectedFileName, _] = parsed;

			if (selectedFileName !== fileName) return;
		}

		this._selectedDiffLines.clear();
		this._selectedDiffFile = undefined;
		this.startIndex = undefined;
		this._quote = false;
	}

	private setSelectedDiffLines(start: number, end: number, params: LineClickParams) {
		const startIndex = Math.min(start, params.index);
		const endIndex = Math.max(start, params.index);

		if (params.rows) {
			this._selectedDiffLines.clear();

			for (let i = startIndex; i <= endIndex; i++) {
				const row = params.rows[i];
				const key = createDiffLineKey(i, row.beforeLineNumber, row.afterLineNumber);
				this._selectedDiffLines.add(key);
			}
		}
	}

	toggle(fileName: string, diffSha: string, params: LineClickParams) {
		const diffFileHunkKey = createDiffFileHunkKey(fileName, diffSha);

		if (this._selectedDiffFile !== diffFileHunkKey) {
			this._selectedDiffLines.clear();
			this._selectedDiffFile = diffFileHunkKey;
		}

		const key = createDiffLineKey(params.index, params.oldLine, params.newLine);
		const isOnlyOneSelected =
			this._selectedDiffLines.size === 1 && this._selectedDiffLines.has(key);

		if (this.startIndex === undefined && !params.shift) {
			this.startIndex = params.startIndex;
		}

		// Handle shift selection
		if (params.shift && this.startIndex !== undefined) {
			this.setSelectedDiffLines(this.startIndex, params.index, params);
			return;
		}

		// Handle new selection.
		// We can tell is a new selection if the index is the same as the start index.
		if (params.index === params.startIndex && !isOnlyOneSelected) {
			this._quote = false;
			this._selectedDiffLines.clear();
			this.startIndex = params.startIndex;
		}

		// Handle drag selection.
		if (params.index !== params.startIndex) {
			this.setSelectedDiffLines(params.startIndex, params.index, params);
			return;
		}

		// Handle single line selection
		if (this._selectedDiffLines.has(key)) {
			this._selectedDiffLines.delete(key);
		} else {
			this._selectedDiffLines.add(key);
		}
	}

	quote() {
		this.chatMinimizer.maximize();
		this._quote = true;
	}

	copy(sections: ContentSection[]) {
		const selectedLines = this.selectedLines;
		if (selectedLines.length === 0) return;

		const flatSectionLines = sections.flatMap((section) => section.lines);

		const buffer: string[] = [];
		for (const line of selectedLines) {
			const sectionLine = flatSectionLines.find(
				(sectionLine) =>
					sectionLine.beforeLineNumber === line.oldLine &&
					sectionLine.afterLineNumber === line.newLine
			);

			if (!sectionLine) continue;

			buffer.push(sectionLine.content);
		}

		const copyString = buffer.join('\n');
		copyToClipboard(copyString);
		this.clear();
	}

	get selectedLines() {
		if (this._quote) return [];
		return this._selectedLines;
	}

	get selectedSha() {
		if (!this._selectedDiffFile) return;

		const parsed = readDiffFileHunkKey(this._selectedDiffFile);
		if (!parsed) return;

		const [_, sha] = parsed;
		return sha;
	}

	get diffSelection(): DiffSelection | undefined {
		if (!this._quote || !this._selectedDiffFile || this._selectedLines.length === 0) return;

		const parsed = readDiffFileHunkKey(this._selectedDiffFile);
		if (!parsed) return;
		const [fileName, diffSha] = parsed;

		return {
			diffSha,
			fileName,
			lines: this._selectedLines
		};
	}
}
