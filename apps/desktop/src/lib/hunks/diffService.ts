import type { Tauri } from '$lib/backend/tauri';
import type { TreeChange } from './change';
import type { UnifiedDiff } from './diff';

export class DiffService {
	constructor(private tauri: Tauri) {}

	/**
	 * Gets the unified diff for a given TreeChange.
	 * This probably does not belong in a package called "worktree" since this also operates on commit-to-commit changes and not only worktree changes
	 */
	async treeChangeDiffs(projectId: string, change: TreeChange) {
		return await this.tauri.invoke<UnifiedDiff>('tree_change_diffs', { projectId, change });
	}
}
