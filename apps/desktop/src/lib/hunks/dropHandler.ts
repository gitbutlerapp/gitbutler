import { ChangeDropData, HunkDropDataV3 } from '$lib/dragging/draggables';
import { type DiffService, type HunkAssignments } from '$lib/hunks/diffService.svelte';
import { hunkHeaderEquals, type HunkAssignmentRequest } from '$lib/hunks/hunk';
import type { DropzoneHandler } from '$lib/dragging/handler';

export class AssignmentDropHandler implements DropzoneHandler {
	constructor(
		private readonly projectId: string,
		private readonly diffService: DiffService,
		private readonly assignments: HunkAssignments,
		private readonly stackId: string
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
			const assignments: HunkAssignmentRequest[] = [];
			for (const file of data.changes) {
				const stackGroup = this.assignments[data.stackId || 'unassigned'];
				if (!stackGroup) continue;
				const fileAssignments = stackGroup[file.path];
				if (fileAssignments) {
					assignments.push(...structuredClone(fileAssignments));
				}
			}
			for (const assignment of assignments) {
				assignment.stackId = this.stackId === 'unassigned' ? null : this.stackId;
			}

			await this.diffService.assignHunk({
				projectId: this.projectId,
				assignments
			});
		} else {
			const selectionId = data.selectionId;
			if (selectionId.type !== 'worktree') {
				throw new Error('Mission impossible');
			}

			const stackGroup = this.assignments[selectionId.stackId || 'unassigned'];
			if (!stackGroup) return;
			const fileAssignments = stackGroup[data.change.path];
			const fileAssignment: HunkAssignmentRequest | undefined = structuredClone(
				fileAssignments?.find(
					(assignment) =>
						assignment.hunkHeader !== null && hunkHeaderEquals(assignment.hunkHeader, data.hunk)
				)
			);
			if (!fileAssignment) return;
			fileAssignment.stackId = this.stackId === 'unassigned' ? null : this.stackId;

			await this.diffService.assignHunk({
				projectId: this.projectId,
				assignments: [fileAssignment]
			});
		}
	}
}
