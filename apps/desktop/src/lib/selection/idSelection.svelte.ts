import {
	selectionKey,
	key,
	readKey,
	type SelectedFileKey,
	type SelectionId
} from '$lib/selection/key';
import { SvelteSet } from 'svelte/reactivity';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';

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

	constructor(private worktreeService: WorktreeService) {
		this.selections = new Map();
		this.selections.set('worktree', {
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
	treeChanges(projectId: string, params: SelectionId) {
		const filePaths = this.values(params).map((fileSelection) => {
			return fileSelection.path;
		});

		return this.worktreeService.getChangesById(projectId, filePaths);
	}

	get length() {
		return this.selections.size;
	}

	collectionSize(params: SelectionId) {
		return this.getById(params).entries.size;
	}
}
