import { key, type SelectionId } from '$lib/selection/key';
import type { CommitDropData } from '$lib/commits/dropHandler';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkAssignment, HunkHeader } from '$lib/hunks/hunk';
import type { IdSelection } from '$lib/selection/idSelection.svelte';

export class HunkDropDataV3 {
	constructor(
		readonly change: TreeChange,
		readonly hunk: HunkHeader,
		readonly uncommitted: boolean,
		readonly stackId: string | null,
		readonly commitId: string | undefined,
		readonly selectionId: SelectionId
	) {}
}

export class ChangeDropData {
	constructor(
		private projectId: string,
		readonly change: TreeChange,
		/**
		 * When a a file is dragged we compare it to what is already selected,
		 * if dragged item is part of the selection we consider that to be to
		 * be dragging all of them. If it is not part of the selection, we
		 * want to ignore what is selected and only drag the actual file being
		 * dragged.
		 */
		private selection: IdSelection,
		readonly selectionId: SelectionId,
		readonly stackId?: string
	) {}

	changedPaths(params: SelectionId): string[] {
		if (this.selection.has(this.change.path, this.selectionId)) {
			return this.selection.keys(params);
		} else {
			return [key({ ...this.selectionId, path: this.change.path })];
		}
	}

	/**
	 * If there is more than one selected item, and the item being dragged is
	 * part of that selection, then a drop handler will take an action on the
	 * whole selection. If, however, the item being dragged is not part of a
	 * selection then any action should be taken on that item alone.
	 */
	async treeChanges(): Promise<TreeChange[]> {
		if (this.selection.has(this.change.path, this.selectionId)) {
			return await this.selection.treeChanges(this.projectId, this.selectionId);
		}
		return [this.change];
	}

	assignments(): Record<string, HunkAssignment[]> | undefined {
		if (this.selection.has(this.change.path, this.selectionId)) {
			return this.selection.hunkAssignments(this.selectionId) ?? undefined;
		}
		return undefined;
	}

	get isCommitted(): boolean {
		return this.selectionId.type === 'commit' || this.selectionId.type === 'branch';
	}
}

export type DropData = CommitDropData | ChangeDropData | HunkDropDataV3;
