import { type TreeChange } from '$lib/hunks/change';
import {
	lineIdsToHunkHeaders,
	type DiffHunk,
	type DiffSpec,
	type HunkAssignment,
	type HunkHeader
} from '$lib/hunks/hunk';
import {
	assignmentSelectors,
	assignmentSlice,
	hunkCheckStatus,
	changeSelectors,
	folderCheckStatus,
	type CheckStatus,
	assignmentActions,
	fileCheckStatus,
	checkboxSelectors,
	type CheckmarkSelection
} from '$lib/selection/assignmentSlice';
import { type Reactive, reactive } from '@gitbutler/shared/storeUtils';
import { type ThunkDispatch, type UnknownAction } from '@reduxjs/toolkit';
import { persistReducer } from 'redux-persist';
import storage from 'redux-persist/es/storage';
import type { DiffService } from '$lib/hunks/diffService.svelte';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

export class AssignmentService {
	/** The change selection slice of the full redux state. */
	private state = $state.raw(assignmentSlice.getInitialState());
	private dispatch: ThunkDispatch<any, any, UnknownAction>;

	constructor(
		clientState: ClientState,
		private worktreeService: WorktreeService,
		private diffService: DiffService
	) {
		this.dispatch = clientState.dispatch;
		const persistConfig = {
			key: assignmentSlice.reducerPath,
			storage: storage
		};

		clientState.inject(
			assignmentSlice.reducerPath,
			persistReducer(persistConfig, assignmentSlice.reducer)
		);

		$effect(() => {
			if (clientState.reactiveState && assignmentSlice.reducerPath in clientState.reactiveState) {
				// @ts-expect-error code-splitting means it's not defined in client state.
				this.state = clientState.reactiveState[assignmentSlice.reducerPath] as IRCState;
			}
		});
	}

	updateAssignments(args: { assignments: HunkAssignment[]; changes: TreeChange[] }) {
		this.dispatch(assignmentActions.updateAssignments(args));
	}

	clearCheckmarks(stackId?: string) {
		this.dispatch(assignmentActions.clearCheckmarks({ stackId: stackId || null }));
	}

	async findHunkDiff(
		projectId: string,
		filePath: string,
		hunk: HunkHeader
	): Promise<DiffHunk | undefined> {
		const treeChange = await this.worktreeService.fetchChange(projectId, filePath);
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

	async worktreeChanges(projectId: string, stackId?: string) {
		const key = `${stackId || null}-`;
		const selection = checkboxSelectors.selectByPrefix(this.state.selections, key);

		const pathGroups = selection.reduce<Record<string, CheckmarkSelection[]>>((acc, item) => {
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
			const change = changeSelectors.selectById(this.state.changes, path)!;
			for (const { lines, assignmentId } of selection) {
				// We want to use `null` to commit from unassigned changes if new stack was created.
				const assignment = assignmentSelectors.selectById(this.state.assignments, assignmentId)!;

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

	selectedChanges(stackId?: string) {
		const result = $derived(changeSelectors.selectedChangesByStackId(this.state, stackId || null));
		return reactive(() => result);
	}

	/**
	 * Returns all assignments along with any line selections.
	 */
	selectedLines(stackId?: string) {
		const key = `${stackId || null}-`;
		const result = $derived(checkboxSelectors.selectByPrefix(this.state.selections, key));
		return reactive(() => result);
	}

	changesByStackId(stackId: string | null): Reactive<TreeChange[]> {
		const changes = $derived(changeSelectors.selectChangesByStackId(this.state, stackId));
		return reactive(() => changes);
	}

	assignmentsByPath(stackId: string | null, path: string): Reactive<HunkAssignment[]> {
		const result = $derived(
			assignmentSelectors.selectByPrefix(this.state.assignments, stackId + '-' + path + '-')
		);
		return reactive(() => result);
	}

	assignmentsByStackId(stackId: string | null): Reactive<HunkAssignment[]> {
		const result = $derived(
			assignmentSelectors.selectByPrefix(this.state.assignments, stackId + '-')
		);
		return reactive(() => result);
	}

	getByStackId(stackId: string | null): Reactive<HunkAssignment[]> {
		const assignments = $derived(
			assignmentSelectors.selectByPrefix(this.state.assignments, String(stackId))
		);
		return reactive(() => assignments);
	}

	getByPath(stackId: string | null, path: string): Reactive<HunkAssignment[]> {
		const assignments = $derived(
			assignmentSelectors.selectByPrefix(this.state.assignments, `${stackId}-${path}-`)
		);
		return reactive(() => assignments);
	}

	getByHeader(
		stackId: string | null,
		path: string,
		header: string
	): Reactive<HunkAssignment | undefined> {
		const assignments = $derived(
			assignmentSelectors.selectById(this.state?.assignments, `${stackId}-${path}-${header}`)
		);
		return reactive(() => assignments);
	}

	hunkCheckStatus(stackId: string | null, path: string, header: string) {
		const result = $derived(hunkCheckStatus(this.state, { stackId, path, header }));
		return reactive(() => result);
	}

	fileCheckStatus(stackId: string | undefined, path: string): Reactive<CheckStatus> {
		const result = $derived(fileCheckStatus(this.state, { stackId: stackId || null, path }));
		return reactive(() => result);
	}

	folderCheckStatus(stackId: string | undefined, prefix: string): Reactive<CheckStatus> {
		const result = $derived(
			folderCheckStatus(this.state, { stackId: stackId || null, path: prefix })
		);
		return reactive(() => result);
	}

	stackCheckStatus(stackId: string | undefined): Reactive<CheckStatus> {
		const result = $derived(folderCheckStatus(this.state, { stackId: stackId || null, path: '' }));
		return reactive(() => result);
	}

	checkLine(stackId: string | null, path: string, hunkHeader: string, line: LineId) {
		this.dispatch(assignmentActions.checkLine({ stackId, path, hunkHeader, line }));
	}

	uncheckLine(stackId: string | null, path: string, header: string, line: LineId) {
		this.dispatch(assignmentActions.uncheckLine({ stackId, path, header, line }));
	}

	checkHunk(stackId: string | null, path: string, header: string) {
		this.dispatch(assignmentActions.checkHunk({ stackId, path, hunkHeader: header }));
	}

	uncheckHunk(stackId: string | null, path: string, header: string) {
		this.dispatch(assignmentActions.uncheckHunk({ stackId, path, hunkHeader: header }));
	}

	checkFile(stackId: string | null, path: string) {
		this.dispatch(assignmentActions.checkFile({ stackId, path }));
	}

	uncheckFile(stackId: string | null, path: string) {
		this.dispatch(assignmentActions.uncheckFile({ stackId, path }));
	}

	checkAll(stackId: string | null) {
		this.dispatch(assignmentActions.checkStack({ stackId }));
	}

	uncheckAll(stackId: string | null) {
		this.dispatch(assignmentActions.uncheckStack({ stackId }));
	}
}
