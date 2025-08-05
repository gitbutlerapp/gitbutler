import { type TreeChange } from '$lib/hunks/change';
import {
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
import { InjectionToken } from '@gitbutler/shared/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { type Reactive } from '@gitbutler/shared/storeUtils';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { type ThunkDispatch, type UnknownAction } from '@reduxjs/toolkit';
import { persistReducer } from 'redux-persist';
import storage from 'redux-persist/es/storage';
import type { ChangeDiff, DiffService } from '$lib/hunks/diffService.svelte';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

export const UNCOMMITTED_SERVICE = new InjectionToken<UncommittedService>('UncommittedService');

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

	async findHunkDiff(
		projectId: string,
		filePath: string,
		hunk: HunkHeader
	): Promise<DiffHunk | undefined> {
		const treeChange = await this.worktreeService.fetchTreeChange(projectId, filePath);
		if (treeChange === undefined) {
			throw new Error('Failed to fetch change');
		}
		const changeDiff = await this.diffService.fetchDiff(projectId, treeChange);
		if (changeDiff === undefined) {
			throw new Error('Failed to fetch diff');
		}
		const file = changeDiff;

		if (file.type !== 'Patch') return undefined;

		const hunkDiff = file.subject.hunks.find(
			(hunkDiff) =>
				hunkDiff.oldStart === hunk.oldStart &&
				hunkDiff.oldLines === hunk.oldLines &&
				hunkDiff.newStart === hunk.newStart &&
				hunkDiff.newLines === hunk.newLines
		);
		return hunkDiff;
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
			const hunkHeaders: HunkHeader[] = [];
			const change = uncommittedSelectors.treeChanges.selectById(state.treeChanges, path)!;
			for (const { lines, assignmentId } of selection) {
				// We want to use `null` to commit from unassigned changes if new stack was created.
				const assignment = uncommittedSelectors.hunkAssignments.selectById(
					state.hunkAssignments,
					assignmentId
				)!;

				if (assignment.hunkHeader !== null) {
					if (lines.length === 0) {
						hunkHeaders.push(assignment.hunkHeader);
						continue;
					} else {
						const hunkDiff = await this.findHunkDiff(
							projectId,
							assignment.path,
							assignment.hunkHeader
						);
						if (!hunkDiff) {
							throw new Error('Hunk not found while commiting');
						}
						hunkHeaders.push(...lineIdsToHunkHeaders(lines, hunkDiff.diff, 'commit'));
						continue;
					}
				}
			}

			hunkHeaders.sort(orderHeaders);

			const status = change.status;
			worktreeChanges.push({
				pathBytes: change.pathBytes,
				previousPathBytes: status.type === 'Rename' ? status.subject.previousPathBytes : null,
				hunkHeaders
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

		return changes;
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
				if (diff.diff.type !== 'Patch') return diff;

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
		const stackIdChanges = uncommittedSelectors.treeChanges.selectByStackId(this.state, stackId);
		return stackIdChanges;
	}

	changesByStackId(stackId: string | null): Reactive<TreeChange[]> {
		const changes = $derived(this.getChangesByStackId(stackId));
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
	startCommitVisible(stackId: string): Reactive<boolean> {
		const assignments = $derived(
			uncommittedSelectors.hunkAssignments.selectByPrefix(
				this.state.hunkAssignments,
				partialKey(stackId)
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

	hunkCheckStatus(stackId: string | null, path: string, header: HunkHeader) {
		const result = $derived(
			uncommittedSelectors.hunkSelection.hunkCheckStatus(this.state, {
				stackId,
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
