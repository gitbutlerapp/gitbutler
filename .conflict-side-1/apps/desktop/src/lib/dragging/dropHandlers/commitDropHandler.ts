import { changesToDiffSpec } from "$lib/commits/utils";
import {
	FileChangeDropData,
	FolderChangeDropData,
	HunkDropDataV3,
	type ChangeDropData,
} from "$lib/dragging/draggables";
import { classify } from "$lib/error/errorClassification";
import { HookFailedError, HOOKS_SERVICE } from "$lib/git/hooksService";
import { toCommitMovePlacement } from "$lib/stacks/commitMovePlacement";
import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
import { UI_STATE, withStackBusy, type UiState } from "$lib/state/uiState.svelte";
import { inject } from "@gitbutler/core/context";
import { untrack } from "svelte";
import type { DropResult } from "$lib/dragging/dropResult";
import type { DropzoneHandler } from "$lib/dragging/handler";
import type { CreateCommitOutcome } from "$lib/stacks/stackEndpoints";
import type { RejectionReason } from "@gitbutler/but-sdk";

/** Details about a commit belonging to a drop zone. */
export type DzCommitData = {
	id: string;
	isRemote: boolean;
	isIntegrated: boolean;
	hasConflicts: boolean;
};

/** Details about a commit that can be dropped into a drop zone. */
export class CommitDropData {
	/** All commits being dragged (for multi-select). Defaults to just `[commit]`. */
	readonly allCommits: DzCommitData[];

	constructor(
		readonly stackId: string,
		readonly commit: DzCommitData,
		readonly isHeadCommit: boolean,
		readonly branchName?: string,
		allCommits?: DzCommitData[],
	) {
		this.allCommits = allCommits && allCommits.length > 0 ? allCommits : [commit];
	}

	get isMultiCommit(): boolean {
		return this.allCommits.length > 1;
	}
}

/** Handler that can move commits between stacks. */
export class MoveCommitDzHandler implements DropzoneHandler {
	private readonly uiState = inject(UI_STATE);
	private readonly stackService = inject(STACK_SERVICE);

	constructor(
		private stackId: string,
		private projectId: string,
		private targetBranchName: string,
	) {}

	accepts(data: unknown): boolean {
		return (
			data instanceof CommitDropData &&
			data.stackId !== this.stackId &&
			!data.allCommits.some((c) => c.hasConflicts)
		);
	}

	async ondrop(data: CommitDropData): Promise<DropResult | void> {
		const { relativeTo, side } = toCommitMovePlacement({
			targetBranchName: this.targetBranchName,
			targetCommitId: "top",
		});

		// Clear the selection from the source lane if any dragged commit was selected
		const sourceSelection = untrack(() => this.uiState.lane(data.stackId).selection.current);
		if (
			sourceSelection?.commitId &&
			data.allCommits.some((c) => c.id === sourceSelection.commitId)
		) {
			this.uiState.lane(data.stackId).selection.set(undefined);
		}

		const commitIds = data.allCommits.map((c) => c.id);
		let result: DropResult | undefined;
		await withStackBusy(
			this.uiState,
			this.projectId,
			{ stackIds: [data.stackId, this.stackId] },
			async () => {
				try {
					await this.stackService.commitMove({
						projectId: this.projectId,
						subjectCommitIds: commitIds,
						relativeTo,
						side,
						dryRun: false,
					});
				} catch (error) {
					const classified = classify(error);
					result = {
						type: "warning",
						title: "Cannot move commits",
						message: classified.userMessage ?? classified.message,
					};
				}
			},
		);
		return result;
	}
}

/**
 * Handler that will be able to amend a commit using `TreeChange`.
 */
export class AmendCommitWithChangeDzHandler implements DropzoneHandler {
	private readonly uiState = inject(UI_STATE);
	private readonly stackService = inject(STACK_SERVICE);
	private readonly hooksService = inject(HOOKS_SERVICE);

	constructor(
		private projectId: string,
		private stackId: string,
		private runHooks: boolean,
		private commit: DzCommitData,
		private onresult: (result: string) => void,
	) {}
	accepts(data: unknown): boolean {
		if (!(data instanceof FileChangeDropData || data instanceof FolderChangeDropData)) return false;
		if (this.commit.hasConflicts) return false;
		if (data.selectionId.type === "branch") return false;
		if (data.selectionId.type === "commit" && data.selectionId.commitId === this.commit.id)
			return false;
		return true;
	}

	async ondrop(data: ChangeDropData): Promise<DropResult | void> {
		switch (data.selectionId.type) {
			case "commit": {
				const sourceStackId = data.stackId;
				const sourceCommitId = data.selectionId.commitId;
				const changes = changesToDiffSpec(await data.treeChanges());
				if (sourceStackId && sourceCommitId) {
					await withStackBusy(
						this.uiState,
						this.projectId,
						{
							commitId: sourceCommitId,
							stackIds: [sourceStackId, this.stackId],
						},
						async () => {
							const { workspace } = await this.stackService.moveChangesBetweenCommits({
								projectId: this.projectId,
								destinationStackId: this.stackId,
								destinationCommitId: this.commit.id,
								sourceStackId,
								sourceCommitId,
								changes,
								dryRun: false,
							});

							// Update the project state to point to the new commit if needed.
							updateUiState(this.uiState, sourceStackId, sourceCommitId, workspace.replacedCommits);
							updateUiState(this.uiState, this.stackId, this.commit.id, workspace.replacedCommits);
						},
					);
				} else {
					throw new Error("Change drop data must specify the source stackId");
				}
				break;
			}
			case "branch":
				console.warn("Moving a branch into a commit is an invalid operation");
				break;
			case "worktree": {
				const assignments = data.assignments();
				const worktreeChanges = changesToDiffSpec(await data.treeChanges(), assignments);

				if (this.runHooks) {
					try {
						await this.hooksService.runPreCommitHooks(this.projectId, worktreeChanges);
					} catch (err) {
						if (err instanceof HookFailedError) return { type: "ok" };
						return { type: "error", title: "Git hook failed", error: err };
					}
				}

				const outcome = await this.stackService.amendCommitMutation({
					projectId: this.projectId,
					stackId: this.stackId,
					commitId: this.commit.id,
					worktreeChanges: worktreeChanges,
					dryRun: false,
				});

				if (outcome.newCommit) {
					this.onresult(outcome.newCommit);
				}

				const rejectionResult = toRejectedChangesResult(this.projectId, outcome);

				if (this.runHooks) {
					await this.hooksService.runPostCommitHooks(this.projectId);
				}

				return rejectionResult;
			}
		}
	}
}

export class UncommitDzHandler implements DropzoneHandler {
	private readonly uiState = inject(UI_STATE);
	private readonly stackService = inject(STACK_SERVICE);

	constructor(
		private projectId: string,
		private readonly assignTo?: string,
	) {}

	accepts(data: unknown): boolean {
		if (data instanceof FileChangeDropData || data instanceof FolderChangeDropData) {
			if (data.selectionId.type !== "commit") return false;
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
		if (data instanceof FileChangeDropData || data instanceof FolderChangeDropData) {
			switch (data.selectionId.type) {
				case "commit": {
					const stackId = data.stackId;
					const commitId = data.selectionId.commitId;
					if (stackId && commitId) {
						await withStackBusy(
							this.uiState,
							this.projectId,
							{ commitId, stackIds: [stackId] },
							async () => {
								const changes = changesToDiffSpec(await data.treeChanges());
								const { workspace } = await this.stackService.uncommitChanges({
									projectId: this.projectId,
									stackId,
									commitId,
									changes,
									assignTo: this.assignTo,
									dryRun: false,
								});

								// Update the project state to point to the new commit if needed.
								updateUiState(this.uiState, stackId, commitId, workspace.replacedCommits);
							},
						);
					} else {
						throw new Error("Change drop data must specify the source stackId");
					}
					break;
				}
				case "branch":
					console.warn("Moving a branch into a commit is an invalid operation");
					break;
				case "worktree":
					console.warn("Moving a branch into a commit is an invalid operation");
					break;
			}
		} else {
			if (!(data.stackId && data.commitId)) {
				throw new Error("Can't receive a change without it's source or commit");
			}
			const previousPathBytes =
				data.change.status.type === "Rename" ? data.change.status.subject.previousPathBytes : null;

			const sourceStackId = data.stackId;
			const sourceCommitId = data.commitId;

			await withStackBusy(
				this.uiState,
				this.projectId,
				{ commitId: sourceCommitId, stackIds: [sourceStackId] },
				async () => {
					const { workspace } = await this.stackService.uncommitChanges({
						projectId: this.projectId,
						stackId: sourceStackId,
						commitId: sourceCommitId,
						changes: [
							{
								previousPathBytes,
								pathBytes: data.change.pathBytes,
								hunkHeaders: [
									{
										oldStart: data.hunk.oldStart,
										oldLines: data.hunk.oldLines,
										newStart: data.hunk.newStart,
										newLines: data.hunk.newLines,
									},
								],
							},
						],
						assignTo: this.assignTo,
						dryRun: false,
					});

					// Update the project state to point to the new commit if needed.
					updateUiState(this.uiState, sourceStackId, sourceCommitId, workspace.replacedCommits);
				},
			);

			return;
		}
	}
}

/**
 * Handler that is able to amend a commit using `Hunk`.
 */
export class AmendCommitWithHunkDzHandler implements DropzoneHandler {
	private readonly uiState = inject(UI_STATE);
	private readonly stackService = inject(STACK_SERVICE);
	private readonly hooksService = inject(HOOKS_SERVICE);

	constructor(
		private args: {
			okWithForce: boolean;
			projectId: string;
			stackId: string;
			commit: DzCommitData;
			runHooks: boolean;
		},
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

	async ondrop(data: HunkDropDataV3): Promise<DropResult | void> {
		const { projectId, stackId, commit, okWithForce, runHooks } = this.args;
		if (!okWithForce && commit.isRemote) return;

		if (data instanceof HunkDropDataV3) {
			const previousPathBytes =
				data.change.status.type === "Rename" ? data.change.status.subject.previousPathBytes : null;

			if (!data.uncommitted) {
				if (!(data.stackId && data.commitId)) {
					throw new Error("Can't receive a change without it's source or commit");
				}

				const sourceStackId = data.stackId;
				const sourceCommitId = data.commitId;

				await withStackBusy(
					this.uiState,
					projectId,
					{ commitId: sourceCommitId, stackIds: [sourceStackId, stackId] },
					async () => {
						const { workspace } = await this.stackService.moveChangesBetweenCommits({
							projectId,
							destinationStackId: stackId,
							destinationCommitId: commit.id,
							sourceStackId,
							sourceCommitId,
							changes: [
								{
									previousPathBytes,
									pathBytes: data.change.pathBytes,
									hunkHeaders: [
										{
											oldStart: data.hunk.oldStart,
											oldLines: data.hunk.oldLines,
											newStart: data.hunk.newStart,
											newLines: data.hunk.newLines,
										},
									],
								},
							],
							dryRun: false,
						});

						// Update the project state to point to the new commit if needed.
						updateUiState(this.uiState, sourceStackId, sourceCommitId, workspace.replacedCommits);
						updateUiState(this.uiState, stackId, commit.id, workspace.replacedCommits);
					},
				);

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
							newLines: data.hunk.newLines,
						},
					],
				},
			];

			if (runHooks) {
				try {
					await this.hooksService.runPreCommitHooks(projectId, worktreeChanges);
				} catch (err) {
					if (err instanceof HookFailedError) return { type: "ok" };
					return { type: "error", title: "Git hook failed", error: err };
				}
			}
			const outcome = await this.stackService.amendCommitMutation({
				projectId,
				stackId,
				commitId: commit.id,
				worktreeChanges,
				dryRun: false,
			});

			const rejectionResult = toRejectedChangesResult(projectId, outcome);

			if (runHooks) {
				await this.hooksService.runPostCommitHooks(projectId);
			}

			return rejectionResult;
		}
	}
}

/**
 * Handler that is able to squash two commits using `DzCommitData`.
 */
export class SquashCommitDzHandler implements DropzoneHandler {
	private readonly uiState = inject(UI_STATE);
	private readonly stackService = inject(STACK_SERVICE);

	constructor(
		private args: {
			projectId: string;
			stackId: string;
			commit: DzCommitData;
		},
	) {}

	accepts(data: unknown): boolean {
		const { stackId, commit } = this.args;
		if (!(data instanceof CommitDropData)) return false;
		if (data.stackId !== stackId) return false;

		if (commit.hasConflicts) return false;
		if (data.allCommits.some((c) => c.hasConflicts)) return false;

		// Don't show dropzone on any of the commits being dragged
		if (data.allCommits.some((c) => c.id === commit.id)) return false;

		return true;
	}

	async ondrop(data: unknown) {
		const { projectId, stackId, commit } = this.args;
		if (data instanceof CommitDropData) {
			const sourceCommitIds = data.allCommits.map((c) => c.id).filter((id) => id !== commit.id);
			if (sourceCommitIds.length === 0) return;

			await withStackBusy(
				this.uiState,
				projectId,
				{ commitId: data.commit.id, stackIds: [stackId] },
				async () => {
					await this.stackService.squashCommits({
						projectId,
						sourceCommitIds,
						targetCommitId: commit.id,
					});
				},
			);
		}
	}
}

function toRejectedChangesResult(
	projectId: string,
	outcome: CreateCommitOutcome,
): DropResult | undefined {
	if (outcome.rejectedChanges.length === 0) return undefined;

	const pathsToRejectedChanges = outcome.rejectedChanges.reduce(
		(acc: Record<string, RejectionReason>, { reason, path }) => {
			acc[path] = reason;
			return acc;
		},
		{},
	);

	return {
		type: "rejectedChanges",
		projectId,
		newCommitId: outcome.newCommit ?? undefined,
		commitTitle: undefined,
		targetBranchName: "",
		pathsToRejectedChanges,
	};
}

function updateUiState(
	uiState: UiState,
	stackId: string,
	commitId: string,
	mapping: Record<string, string>,
) {
	const sourceReplacement = mapping[commitId];
	const sourceState = untrack(() => uiState.lane(stackId).selection.current);
	if (sourceReplacement && sourceState) {
		uiState.lane(stackId).selection.set({ ...sourceState, commitId: sourceReplacement });
	}
}

/**
 * Creates drop handlers for amending and squashing commits.
 * Returns undefined if stackId is not provided (read-only mode).
 */
export function createCommitDropHandlers(args: {
	projectId: string;
	stackId: string | undefined;
	commit: DzCommitData;
	runHooks: boolean;
	onCommitIdChange?: (newCommitId: string) => void;
	okWithForce?: boolean;
}): {
	amendHandler: AmendCommitWithChangeDzHandler | undefined;
	squashHandler: SquashCommitDzHandler | undefined;
	hunkHandler: AmendCommitWithHunkDzHandler | undefined;
} {
	const { stackId, commit, onCommitIdChange, okWithForce = true } = args;

	if (!stackId) {
		return {
			amendHandler: undefined,
			squashHandler: undefined,
			hunkHandler: undefined,
		};
	}

	const amendHandler = new AmendCommitWithChangeDzHandler(
		args.projectId,
		stackId,
		args.runHooks,
		commit,
		(newId) => {
			onCommitIdChange?.(newId);
		},
	);

	const squashHandler = new SquashCommitDzHandler({
		projectId: args.projectId,
		stackId,
		commit,
	});

	const hunkHandler = new AmendCommitWithHunkDzHandler({
		projectId: args.projectId,
		stackId,
		commit,
		okWithForce,
		runHooks: args.runHooks,
	});

	return {
		amendHandler,
		squashHandler,
		hunkHandler,
	};
}
