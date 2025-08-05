import { changesToDiffSpec } from '$lib/commits/utils';
import { ChangeDropData, HunkDropDataV3 } from '$lib/dragging/draggables';
import { type HooksService } from '$lib/hooks/hooksService';
import { untrack } from 'svelte';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { UiState } from '$lib/state/uiState.svelte';

/** Details about a commit beloning to a drop zone. */
export type DzCommitData = {
	id: string;
	isRemote: boolean;
	isIntegrated: boolean;
	hasConflicts: boolean;
};

/** Details about a commit that can be dropped into a drop zone. */
export class CommitDropData {
	constructor(
		readonly stackId: string,
		readonly commit: DzCommitData,
		readonly isHeadCommit: boolean,
		readonly branchName?: string
	) {}
}

/** Handler that can move commits between stacks. */
export class MoveCommitDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private stackId: string,
		private projectId: string
	) {}

	accepts(data: unknown): boolean {
		return (
			data instanceof CommitDropData && data.stackId !== this.stackId && !data.commit.hasConflicts
		);
	}
	ondrop(data: CommitDropData): void {
		this.stackService.moveCommit({
			projectId: this.projectId,
			targetStackId: this.stackId,
			commitId: data.commit.id,
			sourceStackId: data.stackId
		});
	}
}

/**
 * Handler that will be able to amend a commit using `TreeChange`.
 */
export class AmendCommitWithChangeDzHandler implements DropzoneHandler {
	constructor(
		private projectId: string,
		private readonly stackService: StackService,
		private readonly hooksService: HooksService,
		private stackId: string,
		private runHooks: boolean,
		private commit: DzCommitData,
		private onresult: (result: string) => void,
		private readonly uiState: UiState
	) {}
	accepts(data: unknown): boolean {
		if (!(data instanceof ChangeDropData)) return false;
		if (this.commit.hasConflicts) return false;
		if (data.selectionId.type === 'branch') return false;
		if (data.selectionId.type === 'commit' && data.selectionId.commitId === this.commit.id)
			return false;
		return true;
	}

	async ondrop(data: ChangeDropData) {
		switch (data.selectionId.type) {
			case 'commit': {
				const sourceStackId = data.stackId;
				const sourceCommitId = data.selectionId.commitId;
				const changes = changesToDiffSpec(await data.treeChanges());
				if (sourceStackId && sourceCommitId) {
					const { replacedCommits } = await this.stackService.moveChangesBetweenCommits({
						projectId: this.projectId,
						destinationStackId: this.stackId,
						destinationCommitId: this.commit.id,
						sourceStackId,
						sourceCommitId,
						changes
					});

					// Update the project state to point to the new commit if needed.
					updateUiState(this.uiState, sourceStackId, sourceCommitId, replacedCommits);
					updateUiState(this.uiState, this.stackId, this.commit.id, replacedCommits);
				} else {
					throw new Error('Change drop data must specify the source stackId');
				}
				break;
			}
			case 'branch':
				console.warn('Moving a branch into a commit is an invalid operation');
				break;
			case 'worktree': {
				const assignments = data.assignments();
				const worktreeChanges = changesToDiffSpec(await data.treeChanges(), assignments);

				if (this.runHooks) {
					try {
						await this.hooksService.runPreCommitHooks(this.projectId, worktreeChanges);
					} catch {
						return;
					}
				}

				const result = this.onresult(
					await this.stackService.amendCommitMutation({
						projectId: this.projectId,
						stackId: this.stackId,
						commitId: this.commit.id,
						worktreeChanges: worktreeChanges
					})
				);

				if (this.runHooks) {
					await this.hooksService.runPostCommitHooks(this.projectId);
				}
				return result;
			}
		}
	}
}

export class UncommitDzHandler implements DropzoneHandler {
	constructor(
		private projectId: string,
		private readonly stackService: StackService,
		private readonly uiState: UiState,
		private readonly assignTo?: string
	) {}

	accepts(data: unknown): boolean {
		if (data instanceof ChangeDropData) {
			if (data.selectionId.type !== 'commit') return false;
			if (!data.selectionId.commitId) return false;
			if (!data.stackId) return false;
			return true;
		}
		if (data instanceof HunkDropDataV3) {
			if (data.uncommitted) return false;
			if (!data.commitId) return false;
			if (!data.stackId) return false;
			return true;
		}
		return false;
	}

	async ondrop(data: ChangeDropData | HunkDropDataV3) {
		if (data instanceof ChangeDropData) {
			switch (data.selectionId.type) {
				case 'commit': {
					const stackId = data.stackId;
					const commitId = data.selectionId.commitId;
					if (stackId && commitId) {
						const changes = changesToDiffSpec(await data.treeChanges());
						const { replacedCommits } = await this.stackService.uncommitChanges({
							projectId: this.projectId,
							stackId,
							commitId,
							changes,
							assignTo: this.assignTo
						});

						// Update the project state to point to the new commit if needed.
						updateUiState(this.uiState, stackId, commitId, replacedCommits);
					} else {
						throw new Error('Change drop data must specify the source stackId');
					}
					break;
				}
				case 'branch':
					console.warn('Moving a branch into a commit is an invalid operation');
					break;
				case 'worktree':
					console.warn('Moving a branch into a commit is an invalid operation');
					break;
			}
		} else {
			if (!(data.stackId && data.commitId)) {
				throw new Error("Can't receive a change without it's source or commit");
			}
			const previousPathBytes =
				data.change.status.type === 'Rename' ? data.change.status.subject.previousPathBytes : null;

			const { replacedCommits } = await this.stackService.uncommitChanges({
				projectId: this.projectId,
				stackId: data.stackId,
				commitId: data.commitId,
				changes: [
					{
						previousPathBytes,
						pathBytes: data.change.pathBytes,
						hunkHeaders: [
							{
								oldStart: data.hunk.oldStart,
								oldLines: data.hunk.oldLines,
								newStart: data.hunk.newStart,
								newLines: data.hunk.newLines
							}
						]
					}
				],
				assignTo: this.assignTo
			});

			// Update the project state to point to the new commit if needed.
			updateUiState(this.uiState, data.stackId, data.commitId, replacedCommits);

			return;
		}
	}
}

/**
 * Handler that is able to amend a commit using `Hunk`.
 */
export class AmendCommitWithHunkDzHandler implements DropzoneHandler {
	constructor(
		private args: {
			stackService: StackService;
			hooksService: HooksService;
			okWithForce: boolean;
			projectId: string;
			stackId: string;
			commit: DzCommitData;
			uiState: UiState;
			runHooks: boolean;
		}
	) {}

	private acceptsHunkV3(data: unknown): boolean {
		const { commit, okWithForce } = this.args;
		if (!okWithForce && commit.isRemote) return false;
		if (commit.isIntegrated) return false;
		if (data instanceof HunkDropDataV3 && data.commitId === commit.id) return false;
		return data instanceof HunkDropDataV3 && !commit.hasConflicts;
	}

	accepts(data: unknown): boolean {
		return this.acceptsHunkV3(data);
	}

	async ondrop(data: HunkDropDataV3): Promise<void> {
		const { stackService, projectId, stackId, commit, okWithForce, uiState, runHooks } = this.args;
		if (!okWithForce && commit.isRemote) return;

		if (data instanceof HunkDropDataV3) {
			const previousPathBytes =
				data.change.status.type === 'Rename' ? data.change.status.subject.previousPathBytes : null;

			if (!data.uncommitted) {
				if (!(data.stackId && data.commitId)) {
					throw new Error("Can't receive a change without it's source or commit");
				}

				const { replacedCommits } = await stackService.moveChangesBetweenCommits({
					projectId,
					destinationStackId: stackId,
					destinationCommitId: commit.id,
					sourceStackId: data.stackId,
					sourceCommitId: data.commitId,
					changes: [
						{
							previousPathBytes,
							pathBytes: data.change.pathBytes,
							hunkHeaders: [
								{
									oldStart: data.hunk.oldStart,
									oldLines: data.hunk.oldLines,
									newStart: data.hunk.newStart,
									newLines: data.hunk.newLines
								}
							]
						}
					]
				});

				// Update the project state to point to the new commit if needed.
				updateUiState(uiState, data.stackId, data.commitId, replacedCommits);
				updateUiState(uiState, stackId, commit.id, replacedCommits);

				return;
			}

			const worktreeChanges = [
				{
					previousPathBytes,
					pathBytes: data.change.pathBytes,
					hunkHeaders: [
						{
							oldStart: data.hunk.oldStart,
							oldLines: data.hunk.oldLines,
							newStart: data.hunk.newStart,
							newLines: data.hunk.newLines
						}
					]
				}
			];

			if (runHooks) {
				try {
					await this.args.hooksService.runPreCommitHooks(projectId, worktreeChanges);
				} catch {
					return;
				}
			}
			stackService.amendCommitMutation({
				projectId,
				stackId,
				commitId: commit.id,
				worktreeChanges
			});
			await this.args.hooksService.runPostCommitHooks(projectId);
		}
	}
}

/**
 * Handler that is able to squash two commits using `DzCommitData`.
 */
export class SquashCommitDzHandler implements DropzoneHandler {
	constructor(
		private args: {
			stackService: StackService;
			projectId: string;
			stackId: string;
			commit: DzCommitData;
		}
	) {}

	accepts(data: unknown): boolean {
		const { stackId, commit } = this.args;
		if (!(data instanceof CommitDropData)) return false;
		if (data.stackId !== stackId) return false;

		if (commit.hasConflicts || data.commit.hasConflicts) return false;
		if (commit.id === data.commit.id) return false;

		return true;
	}

	async ondrop(data: unknown) {
		const { stackService, projectId, stackId, commit } = this.args;
		if (data instanceof CommitDropData) {
			await stackService.squashCommits({
				projectId,
				stackId,
				sourceCommitIds: [data.commit.id],
				targetCommitId: commit.id
			});
		}
	}
}

function updateUiState(
	uiState: UiState,
	stackId: string,
	commitId: string,
	mapping: [string, string][]
) {
	const sourceReplacement = mapping.find(([before]) => before === commitId);
	const sourceState = untrack(() => uiState.stack(stackId).selection.current);
	if (sourceReplacement && sourceState) {
		uiState.stack(stackId).selection.set({ ...sourceState, commitId: sourceReplacement[1] });
	}
}
