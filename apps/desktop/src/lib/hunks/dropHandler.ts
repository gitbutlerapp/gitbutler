import { ChangeDropData, HunkDropDataV3 } from '$lib/dragging/draggables';
import { type DiffService } from '$lib/hunks/diffService.svelte';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { UncommittedService } from '$lib/selection/uncommittedService.svelte';

export class AssignmentDropHandler implements DropzoneHandler {
	constructor(
		private readonly projectId: string,
		private readonly diffService: DiffService,
		private readonly uncommittedService: UncommittedService,
		private readonly stackId: string | null
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
		if (data instanceof ChangeDropData) {
			await this.diffService.assignHunk({
				projectId: this.projectId,
				assignments: data.changes
					.flatMap(
						(c) => this.uncommittedService.getAssignmentsByPath(data.stackId, c.path).current
					)
					.map((h) => ({ ...h, stackId: this.stackId }))
			});
		} else {
			const assignment = this.uncommittedService.getAssignmentByHeader(
				data.stackId,
				data.change.path,
				`${data.hunk.newStart}`
			).current!;
			await this.diffService.assignHunk({
				projectId: this.projectId,
				assignments: [{ ...assignment, stackId: this.stackId }]
			});
		}
	}
}
