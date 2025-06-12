import { type TreeChange } from '$lib/hunks/change';
import {
	lineIdsToHunkHeaders,
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
import { type Reactive, reactive } from '@gitbutler/shared/storeUtils';
import { type ThunkDispatch, type UnknownAction } from '@reduxjs/toolkit';
import { persistReducer } from 'redux-persist';
import storage from 'redux-persist/es/storage';
import type { DiffService } from '$lib/hunks/diffService.svelte';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

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
		if (treeChange.data === undefined) {
			throw new Error('Failed to fetch change');
		}
		const changeDiff = await this.diffService.fetchDiff(projectId, treeChange.data);
		if (changeDiff.data === undefined) {
			throw new Error('Failed to fetch diff');
		}
		const file = changeDiff.data;

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
	 */
	async worktreeChanges(projectId: string, stackId?: string) {
		const state = structuredClone(this.state);

		const key = partialKey(stackId ?? null);
		const selection = uncommittedSelectors.hunkSelection.selectByPrefix(state.hunkSelection, key);

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
			const status = change.status;
			worktreeChanges.push({
				pathBytes: change.pathBytes,
				previousPathBytes: status.type === 'Rename' ? status.subject.previousPathBytes : null,
				hunkHeaders
			});
		}
		return worktreeChanges;
	}

	async selectedChanges(stackId?: string): Promise<TreeChange[]> {
		const state = structuredClone(this.state);

		const key = partialKey(stackId ?? null);
		const selection = uncommittedSelectors.hunkSelection.selectByPrefix(state.hunkSelection, key);

		const pathSet = new Set<string>();
		for (const item of selection) {
			pathSet.add(item.path);
		}

		const changes = uncommittedSelectors.treeChanges.selectByIds(
			state.treeChanges,
			Array.from(pathSet)
		);

		return changes;
	}

	/**
	 * Returns all assignments along with any line selections.
	 */
	selectedLines(stackId?: string) {
		const key = partialKey(stackId ?? null);
		const result = $derived(
			uncommittedSelectors.hunkSelection.selectByPrefix(this.state.hunkSelection, key)
		);
		return reactive(() => result);
	}

	getChangesByStackId(stackId: string | null): TreeChange[] {
		return uncommittedSelectors.treeChanges.selectByStackId(this.state, stackId);
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

	assignmentsByPath(stackId: string | null, path: string): Reactive<HunkAssignment[]> {
		const assignments = $derived(this.getAssignmentsByPath(stackId, path));
		return reactive(() => assignments);
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
