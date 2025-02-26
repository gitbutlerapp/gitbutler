import { key, splitKey } from './key';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';

/**
 * File selection mechanism based on strings id's.
 */
export class IdSelection {
	private state = $state([] as string[]);

	/**
	 * Each rendered file will check if it's selected when the selction
	 * changes so it's best we make the `has()` lookup O(1).
	 */
	private map = $derived(
		this.state.reduce(
			(acc, obj) => {
				acc[obj] = true;
				return acc;
			},
			{} as Record<string, boolean>
		)
	);

	constructor(private worktreeService: WorktreeService) {}

	add(path: string, commitId?: string) {
		const id = key(path, commitId);
		if (this.map[id]) return;
		this.state.push(id);
	}

	addMany(paths: string[], commitId?: string) {
		for (const path of paths) {
			this.add(path, commitId);
		}
	}

	has(path: string, commitId?: string) {
		return this.map[key(path, commitId)] ?? false;
	}

	set(path: string, commitId?: string) {
		this.state = [];
		this.state.push(key(path, commitId));
	}

	remove(path: string, commitId?: string) {
		this.state.splice(
			this.state.findIndex((k) => k === key(path, commitId)),
			1
		);
	}

	reverse() {
		this.state.reverse();
	}

	clear() {
		this.state = [];
	}

	// TODO: Perhaps remove this? Goto reference for more info.
	firstPath() {
		const key = this.state.at(0);
		if (key) {
			return splitKey(key).path;
		}
	}

	values() {
		return [...this.state].map((c) => {
			return splitKey(c);
		});
	}

	/**
	 * Gets tree changes that correspond to selected id's. Note that the
	 * worktree service call does not trigger any request for data, it
	 * instead reuses the entry from listing if available.
	 * TODO: Should this be able to load even if listing hasn't happened?
	 */
	treeChanges(projectId: string) {
		return this.state
			.map((id) => {
				const file = splitKey(id);
				if (file.commitId !== 'undefined') {
					throw new Error('Changes for commits not implemented');
				}
				return this.worktreeService.getChange(projectId, file.path);
			})
			.filter(isDefined);
	}

	get length() {
		return this.state.length;
	}
}
