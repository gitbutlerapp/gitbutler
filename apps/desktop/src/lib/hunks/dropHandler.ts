import { ChangeDropData } from '$lib/dragging/draggables';
import {
	hunkGroupEquals,
	ungroupedGroup,
	type DiffService,
	type HunkAssignments,
	type HunkGroup
} from '$lib/hunks/diffService.svelte';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { HunkAssignmentRequest } from '$lib/hunks/hunk';

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
		return false;
	}
	async ondrop(data: ChangeDropData) {
		if (!data.group) {
			throw new Error('Mission impossible');
		}

		const assignments: HunkAssignmentRequest[] = [];
		for (const file of data.changes) {
			const stackGroup = this.assignments.get(
				data.group.type === 'ungrouped' ? ungroupedGroup : data.group.stackId
			);
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
	}
}
