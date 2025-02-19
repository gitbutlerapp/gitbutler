import { copyToClipboard } from '@gitbutler/shared/clipboard';
import { SvelteSet } from 'svelte/reactivity';
import type { BrandedId } from '@gitbutler/shared/utils/branding';
import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

type DiffLineKey = BrandedId<'DiffLine'>;
type DiffFileHunkKey = BrandedId<'DiffFileHunk'>;
type DiffLineSelectionString = BrandedId<'DiffLineSelection'>;

function createDiffLineKey(
	index: number,
	oldLine: number | undefined,
	newLine: number | undefined
): DiffLineKey {
	return `${index}-${oldLine ?? ''}-${newLine ?? ''}` as DiffLineKey;
}

type ParsedDiffLineKey = {
	index: number;
	oldLine: number | undefined;
	newLine: number | undefined;
};

function readDiffLineKey(key: DiffLineKey): ParsedDiffLineKey {
	const [index, oldLine, newLine] = key.split('-');
	return {
		index: parseInt(index),
		oldLine: oldLine === '' ? undefined : parseInt(oldLine),
		newLine: newLine === '' ? undefined : parseInt(newLine)
	};
}

function createDiffFileHunkKey(fileName: string, hunkIndex: number): DiffFileHunkKey {
	return `${fileName}-${hunkIndex}` as DiffFileHunkKey;
}

function readDiffFileHunkKey(key: DiffFileHunkKey): [string, number] {
	const [fileName, hunkIndex] = key.split('-');
	return [fileName, parseInt(hunkIndex)];
}

export interface DiffLineSelected extends LineSelector {
	index: number;
}

export interface DiffSelection {
	diffSha: string;
	fileName: string;
	hunkIndex: number;
	lines: DiffLineSelected[];
}

function encodeSingleLineSelection(line: DiffLineSelected): DiffLineSelectionString | undefined {
	if (line.newLine !== undefined) {
		return `R${line.newLine}` as DiffLineSelectionString;
	}

	if (line.oldLine !== undefined) {
		return `L${line.oldLine}` as DiffLineSelectionString;
	}

	return undefined;
}

/**
 * Encode the lines selected from the diff into a string.
 *
 * This function expects to receive a continues selection of lines.
 */
export function encodeLineSelection(
	lineSelection: DiffLineSelected[]
): DiffLineSelectionString | undefined {
	if (lineSelection.length === 0) return undefined;
	if (lineSelection.length === 1) return encodeSingleLineSelection(lineSelection[0]);

	const sortedLines = lineSelection.sort((a, b) => a.index - b.index);
	const firstLine = encodeSingleLineSelection(sortedLines[0]);
	const lastLine = encodeSingleLineSelection(sortedLines[sortedLines.length - 1]);

	if (firstLine === undefined || lastLine === undefined) {
		// This should never happen unless data is corrupted
		throw new Error('Invalid line selection: ' + JSON.stringify(lineSelection));
	}

	return `${firstLine}-${lastLine}` as DiffLineSelectionString;
}

function calculateSelectedLines(selectedDiffLines: SvelteSet<DiffLineKey>): DiffLineSelected[] {
	const parsedLines = Array.from(selectedDiffLines).map((key) => readDiffLineKey(key));

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
	private _quote = $state<boolean>(false);
	private _diffSha = $state<string>();
	private _selectedDiffLines = new SvelteSet<DiffLineKey>();
	private _selectedLines: DiffLineSelected[] = $derived(
		calculateSelectedLines(this._selectedDiffLines)
	);
	private _selectedDiffFileHunk = $state<DiffFileHunkKey>();

	clear() {
		this._selectedDiffLines.clear();
		this._selectedDiffFileHunk = undefined;
		this._diffSha = undefined;
		this._quote = false;
	}

	toggle(fileName: string, hunkIndex: number, diffSha: string, params: LineClickParams) {
		this._diffSha = diffSha;
		const diffFileHunkKey = createDiffFileHunkKey(fileName, hunkIndex);

		if (this._selectedDiffFileHunk !== diffFileHunkKey) {
			this._selectedDiffLines.clear();
			this._selectedDiffFileHunk = diffFileHunkKey;
		}

		const key = createDiffLineKey(params.index, params.oldLine, params.newLine);
		const isOnlyOneSelected =
			this._selectedDiffLines.size === 1 && this._selectedDiffLines.has(key);

		if (params.resetSelection && !isOnlyOneSelected) {
			this._selectedDiffLines.clear();
		}

		if (this._selectedDiffLines.has(key)) {
			this._selectedDiffLines.delete(key);
		} else {
			this._selectedDiffLines.add(key);
		}
	}

	quote() {
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
	}

	get selectedLines() {
		return this._selectedLines;
	}

	get diffSelection(): DiffSelection | undefined {
		if (
			!this._quote ||
			!this._selectedDiffFileHunk ||
			this._selectedLines.length === 0 ||
			this._diffSha === undefined
		)
			return;

		const [fileName, hunkIndex] = readDiffFileHunkKey(this._selectedDiffFileHunk);

		return {
			diffSha: this._diffSha,
			fileName,
			hunkIndex,
			lines: this._selectedLines
		};
	}
}
