import { ChangeDropData, HunkDropDataV3 } from '$lib/dragging/draggables';
import { type DiffService } from '$lib/hunks/diffService.svelte';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { IdSelection } from '$lib/selection/idSelection.svelte';
import type { UncommittedService } from '$lib/selection/uncommittedService.svelte';

export class AssignmentDropHandler implements DropzoneHandler {
	constructor(
		private readonly projectId: string,
		private readonly diffService: DiffService,
		private readonly uncommittedService: UncommittedService,
		private readonly stackId: string | undefined,
		private readonly idSelection: IdSelection
	) {}

	accepts(data: unknown) {
		if (data instanceof ChangeDropData) {
			if (data.isCommitted) return false;
			if (data.stackId === this.stackId) return false;
			return true;
		}
		if (data instanceof HunkDropDataV3) {
			if (!data.uncommitted) return false;
			if (data.selectionId.type !== 'worktree') return false;
			if (data.selectionId.stackId === this.stackId) return false;
			return true;
		}
		return false;
	}

	async ondrop(data: ChangeDropData | HunkDropDataV3) {
		if (data.stackId === this.stackId) return;
		if (data instanceof ChangeDropData) {
			// A whole file.
			const changes = await data.treeChanges();
			const assignments = changes
				.flatMap((c) => this.uncommittedService.getAssignmentsByPath(data.stackId || null, c.path))
				.map((h) => ({ ...h, stackId: this.stackId || null }));
			await this.diffService.assignHunk({
				projectId: this.projectId,
				assignments
			});

			// If files are coming from the uncommitted changes
			this.idSelection.remove(data.change.path, data.selectionId);
		} else {
			const assignment = this.uncommittedService.getAssignmentByHeader(
				data.stackId,
				data.change.path,
				data.hunk
			).current!;
			const allAssignments = this.uncommittedService.getAssignmentsByPath(
				data.stackId,
				data.change.path
			);
			await this.diffService.assignHunk({
				projectId: this.projectId,
				assignments: [{ ...assignment, stackId: this.stackId || null }]
			});

			// If we just moved the last assignment, remove the file from the selection.
			if (allAssignments.length === 1) {
				this.idSelection.remove(data.change.path, data.selectionId);
			}
		}
	}
}
