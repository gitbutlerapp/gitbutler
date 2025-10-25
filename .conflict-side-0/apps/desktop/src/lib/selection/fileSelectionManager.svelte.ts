import {
	selectionKey,
	key,
	readKey,
	createWorktreeSelection,
	type SelectedFileKey,
	type SelectionId,
	type SelectedFile
} from '$lib/selection/key';
import { createBranchRef } from '$lib/utils/branch';
import { InjectionToken } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { SvelteSet } from 'svelte/reactivity';
import { get, writable, type Writable } from 'svelte/store';
import type { HistoryService } from '$lib/history/history';
import type { OplogService } from '$lib/history/oplogService.svelte';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkAssignment } from '$lib/hunks/hunk';
import type { UncommittedService } from '$lib/selection/uncommittedService.svelte';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';

export const FILE_SELECTION_MANAGER = new InjectionToken<FileSelectionManager>(
	'FileSelectionManager'
);

/**
 * File selection mechanism based on strings id's.
 */
export class FileSelectionManager {
	private selections: Map<
		/** Return value of `selectionKey`. */
		string,
		{
			/** This property supports range selection. */
			lastAdded: Writable<
				| {
						/** The index of the file in a sorted list of files. */
						index: number;
						/** The key of the file in the selection. */
						key: SelectedFileKey;
				  }
				| undefined
			>;
			entries: SvelteSet<SelectedFileKey>;
		}
	>;

	constructor(
		private stackService: StackService,
		private uncommittedService: UncommittedService,
		private worktreeService: WorktreeService,
		private oplogService: OplogService,
		private historyService: HistoryService
	) {
		this.selections = new Map();
		this.selections.set(selectionKey(createWorktreeSelection({ stackId: undefined })), {
			entries: new SvelteSet<SelectedFileKey>(),
			lastAdded: writable()
		});
	}

	getById(id: SelectionId) {
		const key = selectionKey(id);
		let set = this.selections.get(key);
		if (!set) {
			set = {
				entries: new SvelteSet<SelectedFileKey>(),
				lastAdded: writable()
			};
			this.selections.set(key, set);
		}
		return set;
	}

	hasItems(id: SelectionId) {
		return this.getById(id).entries.size > 0;
	}

	add(path: string, id: SelectionId, index: number) {
		const selectedKey = key({ ...id, path });
		const selection = this.getById(id);
		selection.lastAdded.set({ index, key: selectedKey });
		selection.entries.add(selectedKey);
	}

	addMany(paths: string[], id: SelectionId, last: { path: string; index: number }) {
		for (const path of paths) {
			this.add(path, id, last.index);
		}

		const selectedKey = key({ ...id, path: last.path });
		const selection = this.getById(id);
		selection.lastAdded.set({ index: last.index, key: selectedKey });
	}

	has(path: string, id: SelectionId) {
		const selection = this.getById(id);
		return selection.entries.has(key({ path, ...id }));
	}

	set(path: string, id: SelectionId, index: number) {
		const selection = this.getById(id);
		selection.entries.clear();
		this.add(path, id, index);
	}

	remove(path: string, id: SelectionId) {
		const selectionKey = key({ path, ...id });
		const selection = this.getById(id);
		selection.entries.delete(selectionKey);
		if (get(selection.lastAdded)?.key === selectionKey) {
			selection.lastAdded.set(undefined);
		}
	}

	clear(selectionId: SelectionId) {
		const selection = this.getById(selectionId);
		selection.entries.clear();
		selection.lastAdded.set(undefined);
	}

	clearPreview(selectionId: SelectionId) {
		const selection = this.getById(selectionId);
		selection.lastAdded.set(undefined);
	}

	keys(selectionId: SelectionId) {
		const selection = this.getById(selectionId);
		return Array.from(selection.entries);
	}

	values(params: SelectionId) {
		return this.keys(params).map((key) => readKey(key));
	}

	valuesReactive(params: SelectionId) {
		const selection = this.getById(params);
		const keys = $derived(Array.from(selection.entries).map(readKey));
		return reactive(() => keys);
	}

	/**
	 * Gets tree changes that correspond to selected id's. Note that the
	 * worktree service call does not trigger any request for data, it
	 * instead reuses the entry from listing if available.
	 * TODO: Should this be able to load even if listing hasn't happened?
	 */
	async treeChanges(projectId: string, params: SelectionId): Promise<TreeChange[]> {
		const paths = this.values(params).map((fileSelection) => {
			return fileSelection.path;
		});

		switch (params.type) {
			case 'worktree':
				return this.uncommittedService
					.getChangesByStackId(params.stackId || null)
					.filter((c) => paths.includes(c.path));
			case 'branch': {
				const { remote, branchName } = params;
				const branch = createBranchRef(branchName, remote);
				return await this.stackService.branchChangesByPaths({
					projectId,
					stackId: params.stackId,
					branch,
					paths: paths
				});
			}
			case 'commit':
				return await this.stackService.commitChangesByPaths(projectId, params.commitId, paths);
			case 'snapshot':
				// TODO: Use the commented out code once back end support restored!
				// Without this we can't show a multi file context menu for snapshots.
				// if (paths[0]) {
				// 	const change = await this.oplogService.fetchDiffWorktreeByPath({
				// 		projectId,
				// 		snapshotId: params.snapshotId,
				// 		path: paths[0]
				// 	});
				// 	return change ? [change] : [];
				// }
				return [];
		}
	}

	/**
	 * Retrieve the hunk assignments for the current selection.
	 *
	 * Hunk assignments are only relevant when selecting worktree files.
	 * For branches, commits, and snapshots, this will return null.
	 */
	hunkAssignments(params: SelectionId): Record<string, HunkAssignment[]> | null {
		switch (params.type) {
			case 'worktree': {
				const paths = this.values(params).map((fileSelection) => {
					return fileSelection.path;
				});
				return this.uncommittedService.getAssignmentsByPaths(params.stackId || null, paths);
			}
			case 'branch':
			case 'commit':
			case 'snapshot':
				return null;
		}
	}

	get length() {
		return this.selections.size;
	}

	collectionSize(params: SelectionId) {
		return this.getById(params).entries.size;
	}

	/**
	 * Function that discards any selection not present in the input array.
	 *
	 * This should be called when the back end pushes a new state of the
	 * current worktree changes. Note that this function is a special case
	 * for a particular key. It feels a bit out of place.
	 */
	retain(paths: string[] | undefined) {
		if (paths === undefined) {
			this.selections.clear();
			return;
		}
		const removedFiles: SelectedFile[] = [];
		const worktreeSelection = this.selections.get(
			selectionKey(createWorktreeSelection({ stackId: undefined }))
		);
		if (!worktreeSelection) return;

		for (const selectedFile of worktreeSelection.entries) {
			const parsedKey = readKey(selectedFile);
			if (!paths.includes(parsedKey.path)) {
				removedFiles.push(parsedKey);
			}
		}
		if (removedFiles.length > 0) {
			this.removeMany(removedFiles);
		}
		// TODO: Is this the right thing to do here?
		// worktreeSelection.lastAdded.set(undefined);
	}

	/**
	 * TODO: Fix these types so we don't have to call `.remove(key.path, key)`.
	 * TODO: Optimise this somehow, reactions are triggered for every loop.
	 */
	removeMany(fileKey: SelectedFile[]) {
		for (const key of fileKey) {
			this.remove(key.path, key);
		}
	}

	changeByKey(projectId: string, selectedFile: SelectedFile) {
		switch (selectedFile.type) {
			case 'commit':
				return this.stackService.commitChange(projectId, selectedFile.commitId, selectedFile.path);
			case 'branch': {
				const { remote, branchName } = selectedFile;
				const branch = createBranchRef(branchName, remote);
				return this.stackService.branchChange({
					projectId,
					stackId: selectedFile.stackId,
					branch,
					path: selectedFile.path
				});
			}
			case 'worktree':
				this.uncommittedService.assignmentsByPath(selectedFile.stackId || null, selectedFile.path);
				return this.worktreeService.treeChangeByPath(projectId, selectedFile.path);
			case 'snapshot':
				return this.historyService.snapshotDiffByPath({
					projectId,
					snapshotId: selectedFile.snapshotId,
					path: selectedFile.path
				});
		}
	}
}
