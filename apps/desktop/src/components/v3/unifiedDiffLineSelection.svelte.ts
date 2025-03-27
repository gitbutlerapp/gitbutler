import { type TreeChange } from '$lib/hunks/change';
import { leftJoinBy, outerJoinBy } from '$lib/utils/array';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { DiffHunk } from '$lib/hunks/hunk';
import type {
	ChangeSelectionService,
	PartiallySelectedFile,
	PartiallySelectedHunk,
	SelectedFile,
	SelectedHunk
} from '$lib/selection/changeSelection.svelte';
import type { LineSelectionParams } from '@gitbutler/ui/hunkDiff/lineSelection.svelte';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

export default class LineSelection {
	private startIndex = $state<number | undefined>(undefined);
	private endIndex = $state<number | undefined>(undefined);
	private selectable = $state<boolean>(false);
	private change = $state<TreeChange | undefined>(undefined);
	private pathData = $derived(
		this.change
			? {
					path: this.change.path,
					pathBytes: this.change.pathBytes
				}
			: undefined
	);

	constructor(private readonly changeSelection: ChangeSelectionService) {}

	setSelectable(value: boolean) {
		this.selectable = value;
	}

	setChange(change: TreeChange) {
		this.change = change;
	}

	toggleStageLines(
		selection: SelectedFile | undefined,
		hunk: DiffHunk,
		params: LineSelectionParams,
		allHunks: DiffHunk[]
	) {
		if (!this.selectable || !this.pathData) return;
		const start = params.index;
		const end = params.index;

		this.startIndex = start;
		this.endIndex = end;

		const [linesSelected, restLines] = this.extractLineIds(params, this.startIndex, this.endIndex);

		if (linesSelected.length === 0) {
			return;
		}

		if (selection === undefined) {
			this.changeSelection.add({
				type: 'partial',
				...this.pathData,
				hunks: [{ type: 'partial', ...hunk, lines: linesSelected }]
			});
			return;
		}

		if (selection.type === 'full') {
			this.handleLineStageInFullSelection(allHunks, hunk, restLines);
			return;
		}

		if (selection.type === 'partial') {
			this.handleLineStageInPartialSelection(selection, hunk, linesSelected, restLines);
			return;
		}

		// If the file is not selected, we need to add the lines to the hunk, the hunk to the file and the file to the selection
	}

	private handleLineStageInPartialSelection(
		selection: PartiallySelectedFile,
		hunk: DiffHunk,
		linesSelected: LineId[],
		restLines: LineId[]
	) {
		const stagedHunk = selection.hunks.find(
			(h) => h.newStart === hunk.newStart && h.oldStart === hunk.oldStart
		);

		// eslint-disable-next-line func-style
		const generateKey = (l: LineId) => `${l.oldLine}-${l.newLine}`;

		// If the hunk is already staged, we need to update the lines
		if (stagedHunk) {
			// Handle existing partial hunk selection
			if (stagedHunk.type === 'partial') {
				this.handleHunkPartiallyStaged(
					stagedHunk,
					linesSelected,
					restLines,
					generateKey,
					selection,
					hunk
				);
				return;
			}

			// Handle existing full hunk selection
			if (stagedHunk.type === 'full') {
				this.handleHunkFullyStaged(selection, hunk, restLines);
				return;
			}
		}

		// If the hunk is not staged, we need to add the hunk to the selection
		this.handleHunkIsNotStaged(selection, hunk, linesSelected);
	}

	private handleHunkIsNotStaged(
		selection: PartiallySelectedFile,
		hunk: DiffHunk,
		linesSelected: LineId[]
	) {
		if (!this.pathData) return;
		const newHunks = selection.hunks.slice();
		newHunks.push({
			type: 'partial',
			...hunk,
			lines: linesSelected
		});

		this.changeSelection.update({
			type: 'partial',
			...this.pathData,
			hunks: newHunks
		});
	}

	private handleHunkFullyStaged(
		selection: PartiallySelectedFile,
		hunk: DiffHunk,
		restLines: LineId[]
	) {
		if (!this.pathData) return;
		const newHunks = selection.hunks.map((h) => {
			if (h.newStart === hunk.newStart && h.oldStart === hunk.oldStart) {
				return {
					...h,
					type: 'partial',
					lines: restLines
				} as const;
			}
			return h;
		});

		this.changeSelection.update({
			type: 'partial',
			...this.pathData,
			hunks: newHunks
		});
	}

	private handleHunkPartiallyStaged(
		stagedHunk: PartiallySelectedHunk,
		linesSelected: LineId[],
		restLines: LineId[],
		generateKey: (l: LineId) => string,
		selection: PartiallySelectedFile,
		hunk: DiffHunk
	) {
		if (!this.pathData) return;
		const existingLines = stagedHunk.lines;

		const newLines = outerJoinBy(existingLines, linesSelected, generateKey);
		const unselectedRestLines = leftJoinBy(restLines, newLines, generateKey);
		const newHunks = selection.hunks
			.map((h) => {
				if (h.newStart === hunk.newStart && h.oldStart === hunk.oldStart) {
					if (newLines.length === 0) {
						return undefined;
					}

					if (unselectedRestLines.length === 0) {
						return {
							...h,
							type: 'full'
						} as const;
					}

					return {
						...h,
						type: 'partial',
						lines: newLines
					} as const;
				}
				return h;
			})
			.filter(isDefined);

		if (newHunks.length === 0) {
			this.changeSelection.remove(this.pathData.path);
			return;
		}

		const fullySelectedHunks = newHunks.every((h) => h.type === 'full');
		const type = fullySelectedHunks ? 'full' : 'partial';
		const hunks = fullySelectedHunks ? [] : newHunks;
		this.changeSelection.update({
			type,
			...this.pathData,
			hunks
		});
	}

	private handleLineStageInFullSelection(
		allHunks: DiffHunk[],
		hunk: DiffHunk,
		restLines: LineId[]
	) {
		if (!this.pathData) return;
		const newHunks: SelectedHunk[] = allHunks.map((h) => {
			if (h.newStart === hunk.newStart && h.oldStart === hunk.oldStart) {
				return {
					...h,
					type: 'partial',
					lines: restLines
				} as const;
			}
			return {
				...h,
				type: 'full'
			};
		});

		this.changeSelection.update({
			type: 'partial',
			...this.pathData,
			hunks: newHunks
		});
	}

	private extractLineIds(
		params: LineSelectionParams,
		start: number,
		end: number
	): [LineId[], LineId[], LineId | undefined] {
		const linesSelected: LineId[] = [];
		const rest: LineId[] = [];
		let firstLineSelected: LineId | undefined;

		const rows = params.rows ?? [];
		for (let i = 0; i < rows.length; i++) {
			const row = rows[i]!;
			if (!row.isDeltaLine) continue;

			if (i === params.startIndex) {
				firstLineSelected = {
					oldLine: row.beforeLineNumber,
					newLine: row.afterLineNumber
				};
			}

			if (i >= start && i <= end) {
				linesSelected.push({
					oldLine: row.beforeLineNumber,
					newLine: row.afterLineNumber
				});
				continue;
			}
			rest.push({
				oldLine: row.beforeLineNumber,
				newLine: row.afterLineNumber
			});
		}

		return [linesSelected, rest, firstLineSelected];
	}
}
