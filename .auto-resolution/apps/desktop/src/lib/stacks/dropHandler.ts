import { filesToOwnership } from '$lib/branches/ownership';
import { ChangeDropData, FileDropData, HunkDropData } from '$lib/dragging/draggables';
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

/** Handler when drop changes on a special outside lanes dropzone. */
export class OutsideLaneDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private projectId: string
	) {}

	accepts(data: unknown) {
		return data instanceof ChangeDropData && !data.isCommitted;
	}

	ondrop(data: HunkDropData | FileDropData) {
		// TODO: Implement logic
		console.warn('Outside lane drop', data);
	}
}
