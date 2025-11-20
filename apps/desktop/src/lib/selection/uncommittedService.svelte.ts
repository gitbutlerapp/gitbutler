import { sortLikeFileTree } from '$lib/files/filetreeV3';
import { isSubmoduleStatus, type TreeChange } from '$lib/hunks/change';
import {
	diffToHunkHeaders,
	hunkHeaderEquals,
	lineIdsToHunkHeaders,
	orderHeaders,
	type DiffHunk,
	type DiffSpec,
	type HunkAssignment,
	type HunkHeader
} from '$lib/hunks/hunk';
import { compositeKey, partialKey, type HunkSelection } from '$lib/selection/entityAdapters';
import {
	uncommittedSelectors,
	uncommittedSlice,
	type CheckboxStatus,
	uncommittedActions
} from '$lib/selection/uncommitted';
import { InjectionToken } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { type Reactive } from '@gitbutler/shared/storeUtils';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { type ThunkDispatch, type UnknownAction } from '@reduxjs/toolkit';
import { persistReducer } from 'redux-persist';
import storage from 'redux-persist/es/storage';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { ChangeDiff, DiffService } from '$lib/hunks/diffService.svelte';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

export const UNCOMMITTED_SERVICE = new InjectionToken<UncommittedService>('UncommittedService');

type PreprocessedHunkHeaderType = 'complete' | 'partial';

interface BasePreprocessedHunkHeader {
	readonly type: PreprocessedHunkHeaderType;
	readonly hunkDiff: DiffHunk;
}

interface CompletePreprocessedHunkHeader extends BasePreprocessedHunkHeader {
	readonly type: 'complete';
	readonly header: HunkHeader;
}

interface PartialPreprocessedHunkHeader extends BasePreprocessedHunkHeader {
	readonly type: 'partial';
	readonly selectedLines: LineId[];
}

type PreprocessedHunkHeader = CompletePreprocessedHunkHeader | PartialPreprocessedHunkHeader;

export class UncommittedService {
	/** The change selection slice of the full redux state. */
	private state = $state.raw(uncommittedSlice.getInitialState());
	private dispatch: ThunkDispatch<any, any, UnknownAction>;

	constructor(
		clientState: ClientState,
		private worktreeService: WorktreeService,
		private diffService: DiffService
	) {
		this.dispatch = clientState.dispatch;
		const persistConfig = {
			key: uncommittedSlice.reducerPath,
			storage: storage
		};

		clientState.inject(
			uncommittedSlice.reducerPath,
			persistReducer(persistConfig, uncommittedSlice.reducer)
		);

		$effect(() => {
			if (clientState.reactiveState && uncommittedSlice.reducerPath in clientState.reactiveState) {
				// @ts-expect-error code-splitting means it's not defined in client state.
				this.state = clientState.reactiveState[uncommittedSlice.reducerPath];
			}
		});
	}

	updateData(args: { assignments: HunkAssignment[]; changes: TreeChange[] }) {
		this.dispatch(uncommittedActions.update(args));
	}

	clearHunkSelection(stackId?: string) {
		this.dispatch(uncommittedActions.clearHunkSelection({ stackId: stackId || null }));
	}

	async getUnifiedDiff(projectId: string, change: TreeChange): Promise<UnifiedDiff> {
		const changeDiff = await this.diffService.fetchDiff(projectId, change);
		if (!changeDiff) {
			throw new Error('Failed to fetch diff');
		}
		return changeDiff;
	}

	findHunkDiff(changeDiff: UnifiedDiff, hunk: HunkHeader): DiffHunk | undefined {
		if (changeDiff?.type !== 'Patch') return undefined;

		const hunkDiff = changeDiff.subject.hunks.find(
			(hunkDiff) =>
				hunkDiff.oldStart === hunk.oldStart &&
				hunkDiff.oldLines === hunk.oldLines &&
				hunkDiff.newStart === hunk.newStart &&
				hunkDiff.newLines === hunk.newLines
		);
		return hunkDiff;
	}

	/**
	 * Check whether the given hunks represent a completely selected file.
	 */
	isCompletelySelectedFile(changeDiff: UnifiedDiff, hunkHeaders: HunkHeader[]): boolean {
		if (changeDiff?.type !== 'Patch') return false;
		const fileHunks = changeDiff.subject.hunks;

		if (fileHunks.length !== hunkHeaders.length) {
			return false;
		}

		for (const hunkHeader of hunkHeaders) {
			const matchingHunk = fileHunks.find(
				(hunkDiff) =>
					hunkDiff.oldStart === hunkHeader.oldStart &&
					hunkDiff.oldLines === hunkHeader.oldLines &&
					hunkDiff.newStart === hunkHeader.newStart &&
					hunkDiff.newLines === hunkHeader.newLines
			);

			if (!matchingHunk) {
				// Hunk from selection not found in actual file hunks
				return false;
			}
		}
		return true;
	}

	processHunkHeaders(
		changeDiff: UnifiedDiff,
		preprocessedHeaders: PreprocessedHunkHeader[]
	): HunkHeader[] {
		const finalHunkHeaders: HunkHeader[] = [];

		// Check if all hunks are completely selected.
		if (preprocessedHeaders.every((h) => h.type === 'complete')) {
			const hunkHeaders = preprocessedHeaders.map((h) => h.header);
			const completelySelected = this.isCompletelySelectedFile(changeDiff, hunkHeaders);
			if (completelySelected) {
				// All hunks in the file are completely selected, return an empty array to indicate
				// that the whole file is selected.
				return [];
			}
		}

		for (const preprocessedHeader of preprocessedHeaders) {
			switch (preprocessedHeader.type) {
				case 'complete': {
					// Turn the complete hunk into a list of 0-anchored hunk headers.
					const generatedHeaders = diffToHunkHeaders(preprocessedHeader.hunkDiff.diff, 'commit');
					finalHunkHeaders.push(...generatedHeaders);
					break;
				}
				case 'partial': {
					// Turn the selected lines into individual 0-anchored hunk headers.
					const generatedHeaders = lineIdsToHunkHeaders(
						preprocessedHeader.selectedLines,
						preprocessedHeader.hunkDiff.diff,
						'commit'
					);
					finalHunkHeaders.push(...generatedHeaders);
					break;
				}
			}
		}

		finalHunkHeaders.sort(orderHeaders);
		return finalHunkHeaders;
	}

	/**
	 * Gathers data for creating a commit, based on what hunks are selected.
	 *
	 * If stackId is undefined, it will return only unassigned changes. If it is
	 * defined, it will return the changes assigned to the stack as well as the
	 * unassigned changes.
	 */
	async worktreeChanges(projectId: string, stackId?: string) {
		const state = structuredClone(this.state);

		const key = partialKey(stackId ?? null);
		const selection = uncommittedSelectors.hunkSelection.selectByPrefix(state.hunkSelection, key);
		// If we are committing from a stack, we also want to include the unassigned changes.
		if (stackId) {
			const nullKey = partialKey(null);
			const nulls = uncommittedSelectors.hunkSelection.selectByPrefix(state.hunkSelection, nullKey);
			selection.push(...nulls);
		}

		const pathGroups = selection.reduce<Record<string, HunkSelection[]>>((acc, item) => {
			const key = `${item.path}`;
			if (!acc[key]) {
				acc[key] = [];
			}
			acc[key].push(item);
			return acc;
		}, {});

		const worktreeChanges: DiffSpec[] = [];
		for (const [path, selection] of Object.entries(pathGroups)) {
			const preprocessedHeaders: PreprocessedHunkHeader[] = [];
			const change = uncommittedSelectors.treeChanges.selectById(state.treeChanges, path)!;

			const status = change.status;
			const previousPathBytes = status.type === 'Rename' ? status.subject.previousPathBytes : null;

			if (selection.length === 0) {
				worktreeChanges.push({
					pathBytes: change.pathBytes,
					previousPathBytes,
					hunkHeaders: []
				});
				continue;
			}

			if (isSubmoduleStatus(status)) {
				// Submodules are always committed as complete changes.
				worktreeChanges.push({
					pathBytes: change.pathBytes,
					previousPathBytes,
					hunkHeaders: []
				});
				continue;
			}

			const changeDiff = await this.getUnifiedDiff(projectId, change);
			for (const { lines, assignmentId } of selection) {
				// We want to use `null` to commit from unassigned changes if new stack was created.
				const assignment = uncommittedSelectors.hunkAssignments.selectById(
					state.hunkAssignments,
					assignmentId
				)!;

				if (assignment.hunkHeader !== null) {
					const hunkDiff = this.findHunkDiff(changeDiff, assignment.hunkHeader);
					if (!hunkDiff) {
						throw new Error('Hunk not found while commiting');
					}

					if (lines.length === 0) {
						// A complete hunk is selected.
						preprocessedHeaders.push({
							type: 'complete',
							header: assignment.hunkHeader,
							hunkDiff
						});
						continue;
					}

					if (hunkDiff)
						// Only some lines withing the hunk are selected.
						preprocessedHeaders.push({
							type: 'partial',
							selectedLines: lines,
							hunkDiff
						});
					continue;
				}
			}

			worktreeChanges.push({
				pathBytes: change.pathBytes,
				previousPathBytes,
				hunkHeaders: await this.processHunkHeaders(changeDiff, preprocessedHeaders)
			});
		}

		return worktreeChanges;
	}

	/**
	 * It should be noted that this method looses hunk and line selection
	 * information.
	 *
	 * If stackId is undefined, it will return only unassigned changes. If it is
	 * defined, it will return the changes assigned to the stack as well as the
	 * unassigned changes.
	 */
	selectedChanges(stackId?: string): TreeChange[] {
		const pathSet = new Set<string>();

		const key = partialKey(stackId ?? null);
		const selection = uncommittedSelectors.hunkSelection.selectByPrefix(
			this.state.hunkSelection,
			key
		);

		for (const item of selection) {
			pathSet.add(item.path);
		}

		if (stackId) {
			const nullKey = partialKey(null);
			const nulls = uncommittedSelectors.hunkSelection.selectByPrefix(
				this.state.hunkSelection,
				nullKey
			);
			for (const item of nulls) {
				pathSet.add(item.path);
			}
		}

		const changes = uncommittedSelectors.treeChanges.selectByIds(
			this.state.treeChanges,
			Array.from(pathSet)
		);

		return sortLikeFileTree(changes);
	}

	/**
	 * Given a list of diffs, filter them out based on the current selection.
	 *
	 * If stackId is undefined, it will filter out hunks that are not unassigned.
	 * If stackId is defined, it will filter out hunks that are not assigned to
	 * that stack AND are not unassigned.
	 *
	 * It should be noted that this function does not _yet_ consider line
	 * selections. Doing so would require re-assembling the hunks.
	 */
	filterDiffsBasedOnSelection(diffs: ChangeDiff[], stackId?: string): ChangeDiff[] {
		const relevantHunks = this.selectedLines(stackId).current;

		return diffs
			.map((diff) => {
				// Drop a whole ChangeDiff if there are no hunks at that path
				// selected.
				const hunksAtPath = relevantHunks.filter((l) => l.path === diff.path);
				if (hunksAtPath.length === 0) return undefined;

				// If the diff is not a patch, we can't/don't need to filter it.
				if (diff.diff?.type !== 'Patch') return diff;

				// Select the diff hunks that are also in the list of relevant hunks.
				const filteredDiff = diff.diff.subject.hunks.filter((h) => {
					return hunksAtPath.some((l) => {
						const assignment = uncommittedSelectors.hunkAssignments.selectById(
							this.state.hunkAssignments,
							l.assignmentId
						);
						if (!assignment?.hunkHeader) return false;

						return hunkHeaderEquals(assignment.hunkHeader, h);
					});
				});

				return {
					...diff,
					diff: {
						...diff.diff,
						subject: {
							...diff.diff.subject,
							hunks: filteredDiff
						}
					}
				};
			})
			.filter(isDefined);
	}

	/**
	 * Returns all assignments along with any line selections. When committing
	 * we combine the hunk selections from the left as well as from the stack.
	 *
	 * TODO: Join the selections in a way that is compatible with the back end.
	 */
	selectedLines(stackId?: string) {
		const globalLines = uncommittedSelectors.hunkSelection.selectByPrefix(
			this.state.hunkSelection,
			partialKey(null)
		);
		const result = $derived(
			// TODO: Rewrite in some more intelligent way.
			globalLines.concat(
				stackId
					? uncommittedSelectors.hunkSelection.selectByPrefix(
							this.state.hunkSelection,
							partialKey(stackId)
						)
					: []
			)
		);
		return reactive(() => result);
	}

	getChangesByStackId(stackId: string | null): TreeChange[] {
		const stackIdChanges = sortLikeFileTree(
			uncommittedSelectors.treeChanges.selectByStackId(this.state, stackId)
		);
		return stackIdChanges;
	}

	changesByStackId(stackId: string | null): Reactive<TreeChange[]> {
		const changes = $derived(sortLikeFileTree(this.getChangesByStackId(stackId)));
		return reactive(() => changes);
	}

	getAssignmentsByPath(stackId: string | null, path: string): HunkAssignment[] {
		return uncommittedSelectors.hunkAssignments.selectByPrefix(
			this.state.hunkAssignments,
			partialKey(stackId, path)
		);
	}

	getAssignmentsByPaths(stackId: string | null, paths: string[]): Record<string, HunkAssignment[]> {
		const assignments: Record<string, HunkAssignment[]> = {};
		for (const path of paths) {
			assignments[path] = this.getAssignmentsByPath(stackId, path);
		}
		return assignments;
	}

	getAssignmentsByStackId(stackId: string): HunkAssignment[] {
		return uncommittedSelectors.hunkAssignments.selectByPrefix(
			this.state.hunkAssignments,
			partialKey(stackId)
		);
	}

	assignmentsByPath(stackId: string | null, path: string): Reactive<HunkAssignment[]> {
		const assignments = $derived(this.getAssignmentsByPath(stackId, path));
		return reactive(() => assignments);
	}

	/**
	 * We can hide the commit button when there are no unassigned commits, and
	 * no assigned commits.
	 */
	startCommitVisible(stackId?: string): Reactive<boolean> {
		const assignments = $derived(
			uncommittedSelectors.hunkAssignments.selectByPrefix(
				this.state.hunkAssignments,
				partialKey(stackId || null)
			)
		);
		const unassigned = $derived(
			uncommittedSelectors.hunkAssignments.selectByPrefix(
				this.state.hunkAssignments,
				partialKey(null)
			)
		);
		return reactive(() => assignments.length + unassigned.length > 0);
	}

	getAssignmentByHeader(
		stackId: string | null,
		path: string,
		hunkHeader: HunkHeader
	): Reactive<HunkAssignment | undefined> {
		const assignments = $derived(
			uncommittedSelectors.hunkAssignments.selectById(
				this.state?.hunkAssignments,
				compositeKey({ stackId, path, hunkHeader })
			)
		);
		return reactive(() => assignments);
	}

	hunkCheckStatus(stackId: string | undefined, path: string, header: HunkHeader) {
		const result = $derived(
			uncommittedSelectors.hunkSelection.hunkCheckStatus(this.state, {
				stackId: stackId || null,
				path,
				hunkHeader: header
			})
		);
		return reactive(() => result);
	}

	fileCheckStatus(stackId: string | undefined, path: string): Reactive<CheckboxStatus> {
		const result = $derived(
			uncommittedSelectors.hunkSelection.fileCheckStatus(this.state, {
				stackId: stackId || null,
				path
			})
		);
		return reactive(() => result);
	}

	folderCheckStatus(stackId: string | undefined, prefix: string): Reactive<CheckboxStatus> {
		const result = $derived(
			uncommittedSelectors.hunkSelection.folderCheckStatus(this.state, {
				stackId: stackId || null,
				path: prefix
			})
		);
		return reactive(() => result);
	}

	stackCheckStatus(stackId: string | undefined): Reactive<CheckboxStatus> {
		const result = $derived(
			uncommittedSelectors.hunkSelection.stackCheckStatus(this.state, {
				stackId: stackId || null
			})
		);
		return reactive(() => result);
	}

	checkLine(stackId: string | null, path: string, hunkHeader: HunkHeader, line: LineId) {
		this.dispatch(uncommittedActions.checkLine({ stackId, path, hunkHeader, line }));
	}

	uncheckLine(
		stackId: string | null,
		path: string,
		header: HunkHeader,
		line: LineId,
		allLinesInHunk: LineId[]
	) {
		this.dispatch(
			uncommittedActions.uncheckLine({ stackId, path, hunkHeader: header, line, allLinesInHunk })
		);
	}

	checkHunk(stackId: string | null, path: string, header: HunkHeader) {
		this.dispatch(uncommittedActions.checkHunk({ stackId, path, hunkHeader: header }));
	}

	uncheckHunk(stackId: string | null, path: string, header: HunkHeader) {
		this.dispatch(uncommittedActions.uncheckHunk({ stackId, path, hunkHeader: header }));
	}

	checkFile(stackId: string | null, path: string) {
		this.dispatch(uncommittedActions.checkFile({ stackId, path }));
	}

	checkFiles(stackId: string | null, paths: string[]) {
		this.dispatch(uncommittedActions.checkFiles({ stackId, paths }));
	}

	uncheckFile(stackId: string | null, path: string) {
		this.dispatch(uncommittedActions.uncheckFile({ stackId, path }));
	}

	checkDir(stackId: string | null, path: string) {
		this.dispatch(uncommittedActions.checkDir({ stackId, path }));
	}

	uncheckDir(stackId: string | null, path: string) {
		this.dispatch(uncommittedActions.uncheckDir({ stackId, path }));
	}

	checkAll(stackId: string | null) {
		this.dispatch(uncommittedActions.checkStack({ stackId }));
	}

	uncheckAll(stackId: string | null) {
		this.dispatch(uncommittedActions.uncheckStack({ stackId }));
	}
}
