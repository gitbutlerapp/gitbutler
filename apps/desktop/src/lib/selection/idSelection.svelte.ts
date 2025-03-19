import { key, readKey, type SelectedFileKey, type SelectionParameters } from '$lib/selection/key';
import { SvelteSet } from 'svelte/reactivity';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';

/**
 * File selection mechanism based on strings id's.
 */
export class IdSelection {
	private _lastAddedIndex = $state<number>();
	private selection: SvelteSet<SelectedFileKey>;

	constructor(private worktreeService: WorktreeService) {
		this.selection = new SvelteSet<SelectedFileKey>();
	}

	add(path: string, params: SelectionParameters, index: number) {
		const id = key({ ...params, path });
		this._lastAddedIndex = index;
		this.selection.add(id);
	}

	addMany(paths: string[], params: SelectionParameters, index: number) {
		for (const path of paths) {
			this.add(path, params, index);
		}
	}

	has(path: string, params: SelectionParameters) {
		return this.selection.has(key({ path, ...params }));
	}

	set(path: string, params: SelectionParameters, index: number) {
		this.selection.clear();
		this.add(path, params, index);
	}

	remove(path: string, params: SelectionParameters) {
		const selectionKey = key({ path, ...params });
		this.selection.delete(selectionKey);
	}

	clear() {
		this.selection.clear();
	}

	keys() {
		return Array.from(this.selection);
	}

	values() {
		return this.keys().map((key) => readKey(key));
	}

	/**
	 * Gets tree changes that correspond to selected id's. Note that the
	 * worktree service call does not trigger any request for data, it
	 * instead reuses the entry from listing if available.
	 * TODO: Should this be able to load even if listing hasn't happened?
	 */
	treeChanges(projectId: string) {
		const filePaths = this.values().map((fileSelection) => {
			if (fileSelection.type !== 'worktree') {
				throw new Error('???');
			}
			return fileSelection.path;
		});

		return this.worktreeService.getChangesById(projectId, filePaths);
	}

	get length() {
		return this.selection.size;
	}

	get lastAddedIndex() {
		return this._lastAddedIndex;
	}
}
