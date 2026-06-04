import { renameBranchInHeadInfo, resolveRelativeTo } from "#ui/api/ref-info.ts";
import { encodeRefName } from "#ui/api/ref-name.ts";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { shortCommitId } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { rejectedChangesToastOptions } from "#ui/operations/rejectedChangesToastOptions.tsx";
import { type BranchOperand } from "#ui/operands.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { Toast } from "@base-ui/react";
import {
	type BottomUpdate,
	type CommitAbsorption,
	type InsertSide,
	type RelativeTo,
	type Snapshot,
} from "@gitbutler/but-sdk";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";

export const useAbsorb = ({ projectId }: { projectId: string }) => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (absorptionPlan: Array<CommitAbsorption> | undefined) => {
			if (!absorptionPlan) return Promise.resolve(null);
			return window.lite.absorb({ projectId, absorptionPlan });
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to absorb",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useApplyBranch = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.apply,
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to apply branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitAmend = ({ projectId }: { projectId: string }) => {
	const toastManager = Toast.useToastManager();
	const queryClient = useQueryClient();
	const dispatch = useAppDispatch();

	return useMutation({
		mutationFn: async ({ relativeTo }: { relativeTo: RelativeTo }) => {
			const headInfo = await queryClient.fetchQuery(headInfoQueryOptions(projectId));

			const commitId = resolveRelativeTo({
				headInfo,
				relativeTo,
			});
			if (commitId === null) throw new Error("No commit to amend.");

			const worktreeChanges = await queryClient.fetchQuery(
				changesInWorktreeQueryOptions(projectId),
			);
			const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));

			return await window.lite.commitAmend({
				projectId,
				commitId,
				changes,
				dryRun: false,
			});
		},
		onSuccess: async (response, input, _ctx, { client }) => {
			client.setQueryData(headInfoQueryOptions(projectId).queryKey, response.workspace.headInfo);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);

			if (input.relativeTo.type === "commit" && response.newCommit !== null)
				dispatch(
					projectActions.setCommitTarget({
						projectId,
						commitTarget: { type: "commit", subject: response.newCommit },
					}),
				);

			if (response.rejectedChanges.length > 0)
				toastManager.add(
					rejectedChangesToastOptions({
						newCommit: response.newCommit,
						rejectedChanges: response.rejectedChanges,
					}),
				);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to amend commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitCreate = ({ projectId }: { projectId: string }) => {
	const toastManager = Toast.useToastManager();
	const queryClient = useQueryClient();
	const dispatch = useAppDispatch();

	return useMutation({
		mutationFn: async ({ message, relativeTo }: { message: string; relativeTo: RelativeTo }) => {
			const worktreeChanges = await queryClient.fetchQuery(
				changesInWorktreeQueryOptions(projectId),
			);
			const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));

			return await window.lite.commitCreate({
				projectId,
				relativeTo,
				changes,
				side: Match.value(relativeTo).pipe(
					Match.withReturnType<InsertSide>(),
					Match.when({ type: "commit" }, () => "above"),
					Match.when({ type: "reference" }, () => "below"),
					Match.when({ type: "referenceBytes" }, () => "below"),
					Match.exhaustive,
				),
				message,
				dryRun: false,
			});
		},
		onSuccess: async (response, input) => {
			if (input.relativeTo.type === "commit" && response.newCommit !== null)
				dispatch(
					projectActions.setCommitTarget({
						projectId,
						commitTarget: { type: "commit", subject: response.newCommit },
					}),
				);

			if (response.rejectedChanges.length > 0)
				toastManager.add(
					rejectedChangesToastOptions({
						newCommit: response.newCommit,
						rejectedChanges: response.rejectedChanges,
					}),
				);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitDiscard = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitDiscard,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to discard commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitInsertBlank = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitInsertBlank,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to insert commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitMove = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitMove,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to move commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitReword = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitReword,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to reword commit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitUncommit = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitUncommit,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to uncommit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommitUncommitChanges = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitUncommitChanges,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to uncommit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const usePushStack = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.pushStack,
		onSuccess: async (response, input, _context, mutation) => {
			await mutation.client.invalidateQueries({
				queryKey: headInfoQueryOptions(input.projectId).queryKey,
			});

			if (response.branchToRemote.length === 0) return;

			toastManager.add({
				type: "success",
				title: `Pushed ${response.branchToRemote.length} ${
					new Intl.PluralRules(undefined).select(response.branchToRemote.length) === "one"
						? "branch"
						: "branches"
				}`,
				description: input.branch,
			});
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to push",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRebaseAllStacks = ({ projectId }: { projectId: string }) => {
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (updates: Array<BottomUpdate>) =>
			window.lite.workspaceIntegrateUpstream({ projectId, updates, dryRun: false }),
		onSuccess: (workspace) => {
			queryClient.setQueryData(headInfoQueryOptions(projectId).queryKey, workspace.headInfo);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId,
					replacedCommits: workspace.replacedCommits,
					headInfo: workspace.headInfo,
				}),
			);

			toastManager.add({
				type: "success",
				title: "Rebased all stacks",
			});
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to rebase stacks",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRebaseStack = ({ projectId }: { projectId: string }) => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (update: BottomUpdate) =>
			window.lite.workspaceIntegrateUpstream({ projectId, updates: [update], dryRun: false }),
		onSuccess: (workspace, _input, _context, mutation) => {
			mutation.client.setQueryData(headInfoQueryOptions(projectId).queryKey, workspace.headInfo);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId,
					replacedCommits: workspace.replacedCommits,
					headInfo: workspace.headInfo,
				}),
			);

			toastManager.add({
				type: "success",
				title: "Rebased stack",
			});
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to rebase stack",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRemoveBranch = () => {
	const toastManager = Toast.useToastManager();

	// TODO: This mutation doesn't trigger any watcher events, hence the manual invalidation.
	return useMutation({
		mutationFn: window.lite.removeBranch,
		onSuccess: async (_response, input, _context, mutation) => {
			await mutation.client.invalidateQueries({
				queryKey: headInfoQueryOptions(input.projectId).queryKey,
			});
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to delete branch reference",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRestoreSnapshot = ({ projectId }: { projectId: string }) => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: async (direction: "redo" | "undo"): Promise<Snapshot | null> => {
			const snapshot =
				direction === "redo"
					? await window.lite.getRedoTargetSnapshot(projectId)
					: await window.lite.getUndoTargetSnapshot(projectId);
			if (!snapshot) return null;

			const [peeled] = await Promise.all([
				window.lite.peelRestoreSnapshot({ projectId, sha: snapshot.commitId }),

				window.lite.restoreSnapshotWithKind({
					projectId,
					restoreKind:
						direction === "redo" ? "RestoreFromSnapshotViaRedo" : "RestoreFromSnapshotViaUndo",
					sha: snapshot.commitId,
				}),
			]);

			return peeled ?? snapshot;
		},
		onSuccess: (snapshot, direction) => {
			const title = direction === "redo" ? "Redo" : "Undo";

			if (!snapshot) {
				toastManager.add({ type: "warning", title, description: `Nothing to ${direction}` });
				return;
			}

			// TODO: We should map this to something user-friendly.
			const op = snapshot.details?.operation;

			// TODO: We should use dynamic units.
			const minsAgo = new Intl.RelativeTimeFormat(undefined, { style: "short" }).format(
				Math.ceil((snapshot.createdAt - Date.now()) / 1000 / 60),
				"minutes",
			);

			toastManager.add({
				type: "info",
				title,
				description: `Restored to ${shortCommitId(snapshot.commitId)} (${op !== undefined ? `${op}, ` : ""}${minsAgo})`,
			});
		},
		onError: (error, direction) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: `Failed to ${direction}`,
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useTearOffBranch = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.tearOffBranch,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to tear off branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useUnapplyStack = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.unapplyStack,
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to unapply stack",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useUpdateBranchName = ({
	projectId,
	stackId,
	branchRef,
	oldBranch,
}: {
	projectId: string;
	stackId: string;
	branchRef: Array<number>;
	oldBranch: BranchOperand;
}) => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.updateBranchName,
		onSuccess: async (_response, input, _context, mutation) => {
			const newBranchRef = encodeRefName(`refs/heads/${input.newName}`);
			const newBranch: BranchOperand = {
				stackId,
				// TODO: ideally the API would return the new ref?
				branchRef: newBranchRef,
			};

			mutation.client.setQueryData(headInfoQueryOptions(projectId).queryKey, (headInfo) => {
				if (!headInfo) return headInfo;

				return renameBranchInHeadInfo({
					headInfo,
					stackId,
					branchRef,
					newName: input.newName,
					newBranchRef,
				});
			});

			dispatch(
				projectActions.updateRewrittenBranchReferences({
					projectId,
					oldBranch,
					newBranch,
				}),
			);
			dispatch(projectActions.exitMode({ projectId }));
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to rename branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};
