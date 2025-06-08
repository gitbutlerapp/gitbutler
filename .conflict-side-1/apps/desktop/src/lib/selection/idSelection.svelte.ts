import {
	selectionKey,
	key,
	readKey,
	type SelectedFileKey,
	type SelectionId,
	type SelectedFile
} from '$lib/selection/key';
import { SvelteSet } from 'svelte/reactivity';
import type { TreeChange } from '$lib/hunks/change';
import type { UncommittedService } from '$lib/selection/uncommittedService.svelte';
import type { StackService } from '$lib/stacks/stackService.svelte';

/**
 * File selection mechanism based on strings id's.
 */
export class IdSelection {
	private selections: Map<
		/** Return value of `selectionKey`. */
		string,
		{
			/** This property supports range selection. */
			lastAdded?: number;
			entries: SvelteSet<SelectedFileKey>;
		}
	>;

	constructor(
		private stackService: StackService,
		private uncommittedService: UncommittedService
	) {
		this.selections = new Map();
		this.selections.set(selectionKey({ type: 'worktree' }), {
			entries: new SvelteSet<SelectedFileKey>()
		});
	}

	getById(id: SelectionId) {
		const key = selectionKey(id);
		let set = this.selections.get(key);
		if (!set) {
			set = {
				entries: new SvelteSet<SelectedFileKey>()
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
		selection.lastAdded = index;
		selection.entries.add(selectedKey);
	}

	addMany(paths: string[], id: SelectionId, index: number) {
		for (const path of paths) {
			this.add(path, id, index);
		}
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
	}

	clear(selectionId: SelectionId) {
		const selection = this.getById(selectionId);
		selection.entries.clear();
	}

	keys(selectionId: SelectionId) {
		const selection = this.getById(selectionId);
		return Array.from(selection.entries);
	}

	values(params: SelectionId) {
		return this.keys(params).map((key) => readKey(key));
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
			case 'branch':
				return await this.stackService.branchChangesByPaths({
					projectId,
					stackId: params.stackId,
					branchName: params.branchName,
					paths: paths
				});
			case 'commit':
				return await this.stackService.commitChangesByPaths(projectId, params.commitId, paths);
			case 'snapshot':
				throw new Error('unsupported');
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
		const worktreeSelection = this.selections.get(selectionKey({ type: 'worktree' }));
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
		worktreeSelection.lastAdded = undefined;
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
}
