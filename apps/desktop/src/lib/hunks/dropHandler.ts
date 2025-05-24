import { ChangeDropData, HunkDropDataV3 } from '$lib/dragging/draggables';
import {
	hunkGroupEquals,
	hunkGroupToKey,
	type DiffService,
	type HunkAssignments,
	type HunkGroup
} from '$lib/hunks/diffService.svelte';
import { hunkHeaderEquals, type HunkAssignmentRequest } from '$lib/hunks/hunk';
import type { DropzoneHandler } from '$lib/dragging/handler';

export class AssignmentDropHandler implements DropzoneHandler {
	constructor(
		private readonly projectId: string,
		private readonly diffService: DiffService,
		private readonly assignments: HunkAssignments,
		private readonly destinationGroup: HunkGroup
	) {}

	accepts(data: unknown) {
		if (data instanceof ChangeDropData) {
			if (data.isCommitted) return false;
			if (!data.group) return false;
			if (hunkGroupEquals(data.group, this.destinationGroup)) return false;
			return true;
		}
		if (data instanceof HunkDropDataV3) {
			if (!data.uncommitted) return false;
			if (data.selectionId.type !== 'worktree') return false;
			if (hunkGroupEquals(data.selectionId.group, this.destinationGroup)) return false;
			return true;
		}
		return false;
	}
	async ondrop(data: ChangeDropData | HunkDropDataV3) {
		if (data instanceof ChangeDropData) {
			if (!data.group) {
				throw new Error('Mission impossible');
			}

			const assignments: HunkAssignmentRequest[] = [];
			for (const file of data.changes) {
				const stackGroup = this.assignments.get(hunkGroupToKey(data.group));
				if (!stackGroup) continue;
				const fileAssignments = stackGroup.get(file.path);
				if (fileAssignments) {
					assignments.push(...structuredClone(fileAssignments));
				}
			}
			for (const assignment of assignments) {
				assignment.stackId =
					this.destinationGroup.type === 'ungrouped' ? null : this.destinationGroup.stackId;
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

			const stackGroup = this.assignments.get(hunkGroupToKey(selectionId.group));
			if (!stackGroup) return;
			const fileAssignments = stackGroup.get(data.change.path);
			const fileAssignment: HunkAssignmentRequest | undefined = structuredClone(
				fileAssignments?.find(
					(assignment) =>
						assignment.hunkHeader !== null && hunkHeaderEquals(assignment.hunkHeader, data.hunk)
				)
			);
			if (!fileAssignment) return;
			fileAssignment.stackId =
				this.destinationGroup.type === 'ungrouped' ? null : this.destinationGroup.stackId;

			await this.diffService.assignHunk({
				projectId: this.projectId,
				assignments: [fileAssignment]
			});
		}
	}
}
