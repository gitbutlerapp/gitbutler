import { SvelteSet } from 'svelte/reactivity';
import type { BrandedId } from '@gitbutler/shared/utils/branding';
import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
import type { LineSelector } from '@gitbutler/ui/utils/diffParsing';

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

function readDiffLineKey(key: DiffLineKey): [number, number | undefined, number | undefined] {
	const [index, oldLine, newLine] = key.split('-');
	return [
		parseInt(index),
		oldLine === '' ? undefined : parseInt(oldLine),
		newLine === '' ? undefined : parseInt(newLine)
	];
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

export default class DiffLineSelection {
	private _selectedDiffLines = new SvelteSet<DiffLineKey>();
	private _selectedLines: DiffLineSelected[] = $derived.by(() => {
		return Array.from(this._selectedDiffLines).map((key) => {
			const [index, oldLine, newLine] = readDiffLineKey(key);
			return { index, oldLine, newLine };
		});
	});
	private _selectedDiffFileHunk = $state<DiffFileHunkKey>();

	toggle(fileName: string, hunkIndex: number, params: LineClickParams) {
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

	get selectedLines() {
		return this._selectedLines;
	}

	get diffSelection(): DiffSelection | undefined {
		if (!this._selectedDiffFileHunk || this._selectedLines.length === 0) return;
		const [fileName, hunkIndex] = readDiffFileHunkKey(this._selectedDiffFileHunk);

		return {
			fileName,
			hunkIndex,
			lines: this._selectedLines
		};
	}
}
