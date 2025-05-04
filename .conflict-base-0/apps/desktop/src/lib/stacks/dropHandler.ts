import { filesToOwnership } from '$lib/branches/ownership';
import { FileDropData, HunkDropData } from '$lib/dragging/draggables';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { StackService } from '$lib/stacks/stackService.svelte';

/** Handler that creates a new stack from files or hunks. */
export class NewStackDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private projectId: string
	) {}

	accepts(data: unknown) {
		if (data instanceof FileDropData) {
			return !(data.isCommitted || data.files.some((f) => f.locked));
		}
		if (data instanceof HunkDropData) {
			return !(data.isCommitted || data.hunk.locked);
		}
		return false;
	}

	ondrop(data: FileDropData | HunkDropData) {
		if (data instanceof HunkDropData) {
			const ownership = `${data.hunk.filePath}:${data.hunk.id}`;
			this.stackService.newStackMutation({ projectId: this.projectId, branch: { ownership } });
		} else if (data instanceof FileDropData) {
			const ownership = filesToOwnership(data.files);
			this.stackService.newStackMutation({ projectId: this.projectId, branch: { ownership } });
		}
	}
}
