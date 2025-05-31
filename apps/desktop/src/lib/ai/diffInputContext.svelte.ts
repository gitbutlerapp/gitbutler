import { isLockfile } from '@gitbutler/shared/lockfiles';
import type { DiffInput } from '$lib/ai/service';
import type { TreeChange } from '$lib/hunks/change';
import type { ChangeDiff, DiffService } from '$lib/hunks/diffService.svelte';
import type { SelectedFile } from '$lib/selection/key';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';

export type DiffInputContextType = 'commit' | 'change-selection' | 'file-selection';

interface BaseDiffInputContextArgs {
	type: DiffInputContextType;
	projectId: string;
}

interface CommitDiffInputContextArgs extends BaseDiffInputContextArgs {
	type: 'commit';
	/**
	 * The commit Id to fetch the diff for.
	 *
	 * This assumes that this commit is locally available.
	 */
	commitId: string;
}

interface HunkSelectionDiffInputContextArgs extends BaseDiffInputContextArgs {
	type: 'change-selection';
	/**
	 * The selected files to fetch the diff for.
	 */
	selectedChanges: TreeChange[];
}

interface SelectionDiffInputContextArgs extends BaseDiffInputContextArgs {
	type: 'file-selection';
	/**
	 * The selected files to fetch the diff for.
	 */
	selectedFiles: SelectedFile[];
	/**
	 * All the changes in the worktree.
	 */
	changes: TreeChange[];
}

export type DiffInputContextArgs =
	| CommitDiffInputContextArgs
	| HunkSelectionDiffInputContextArgs
	| SelectionDiffInputContextArgs;

export default class DiffInputContext {
	constructor(
		private readonly worktreeService: WorktreeService,
		private readonly diffService: DiffService,
		private readonly stackService: StackService,
		private readonly args: DiffInputContextArgs
	) {}

	private async changes(): Promise<TreeChange[] | null> {
		switch (this.args.type) {
			case 'commit': {
				const commitChangesResult = await this.stackService.fetchCommitChanges(
					this.args.projectId,
					this.args.commitId
				);
				return commitChangesResult.data?.changes ?? null;
			}

			case 'change-selection': {
				return this.args.selectedChanges;
			}

			case 'file-selection': {
				const filePaths = this.filterSelectedFilePaths(this.args.selectedFiles.map((f) => f.path));
				const selectedChanges = this.args.changes.filter((change) =>
					filePaths.includes(change.path)
				);

				if (selectedChanges.length === 0) {
					return null;
				}

				return selectedChanges;
			}
		}
	}

	private filterSelectedFilePaths(filePaths: string[]) {
		return filePaths.filter((p) => !isLockfile(p));
	}

	private async diffs(): Promise<ChangeDiff[] | undefined> {
		const changes = await this.changes();
		if (!changes) return undefined;

		const diffs = await this.diffService.fetchChanges(this.args.projectId, changes);
		return diffs;
	}

	/**
	 * Get the selected diff information in the expected format for the AI service.
	 *
	 * TODO: Account for the line selection. Right now, it will always send the whole hunk.
	 */
	async diffInput(): Promise<DiffInput[] | undefined> {
		const diffs = await this.diffs();
		if (!diffs) return undefined;

		const diffInput: DiffInput[] = [];

		for (const diff of diffs) {
			const filePath = diff.path;
			const diffStringBuffer: string[] = [];

			if (diff.diff.type !== 'Patch') continue;

			for (const hunk of diff.diff.subject.hunks) {
				diffStringBuffer.push(hunk.diff);
			}

			const diffString = diffStringBuffer.join('\n');
			diffInput.push({
				filePath,
				diff: diffString
			});
		}
		return diffInput;
	}
}
