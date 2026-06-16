import { renameBranchInHeadInfo, resolveRelativeTo } from "#ui/api/ref-info.ts";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { shortCommitId } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import {
	discardChangesToastOptions,
	rejectedChangesToastOptions,
} from "#ui/operations/toastOptions.tsx";
import { type BranchOperand } from "#ui/operands.ts";
import { projectActions } from "#ui/projects/state.ts";
import { type AppDispatch, useAppDispatch } from "#ui/store.ts";
import { Toast } from "@base-ui/react";
import {
	type BottomUpdate,
	type CommitAbsorption,
	type InsertSide,
	type RelativeTo,
	type Snapshot,
} from "@gitbutler/but-sdk";
import { type QueryClient, useMutation, useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";
import { BranchCheckoutNewParams, BranchCreateParams, OpenInEditorParams } from "#electron/ipc.ts";

// oxlint-disable-next-line typescript/no-explicit-any
type PromiseReturnType<T> = T extends (...args: Array<any>) => Promise<infer U> ? U : never;
type AnyResponse = PromiseReturnType<(typeof window.lite)[keyof typeof window.lite]>;

export const syncCoreCaches = (
	queryClient: QueryClient,
	dispatch: AppDispatch,
	projectId: string,
	response: Exclude<AnyResponse, void>,
) => {
	if (typeof response !== "object" || response === null || !("workspace" in response)) return;

	queryClient.setQueryData(headInfoQueryOptions(projectId).queryKey, response.workspace.headInfo);
	dispatch(
		projectActions.updateRewrittenCommitReferences({
			projectId,
			replacedCommits: response.workspace.replacedCommits,
			headInfo: response.workspace.headInfo,
		}),
	);
};

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

export const useBranchCreate = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (input: BranchCreateParams) => window.lite.branchCreate(input),
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to create branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useBranchCheckoutNew = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (input: BranchCheckoutNewParams) => window.lite.branchCheckoutNew(input),
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to switch to new branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useOpenInEditor = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (input: OpenInEditorParams) => window.lite.openInEditor(input),
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to open in editor",
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
			syncCoreCaches(client, dispatch, projectId, response);

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
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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

export const useCommitDiscardChanges = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitDiscardChanges,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to discard changes",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useDiscardWorktreeChanges = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.discardWorktreeChanges,
		onSuccess: (rejectedChanges) => {
			if (rejectedChanges.length > 0)
				toastManager.add(discardChangesToastOptions({ rejectedChanges }));
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to discard changes",
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
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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

export const useCommitUncommit = ({ projectId }: { projectId: string }) => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitUncommit,
		onSuccess: async (response, _input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, projectId, response);
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
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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

export const useWorkspaceIntegrateUpstream = ({ projectId }: { projectId: string }) => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (updates: Array<BottomUpdate>) =>
			window.lite.workspaceIntegrateUpstream({ projectId, updates, dryRun: false }),
		onSuccess: (response, updates, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, projectId, response);

			toastManager.add({
				type: "success",
				title: updates.length === 1 ? "Updated stack" : "Updated stacks",
			});
		},
		onError: (error, updates) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: updates.length === 1 ? "Failed to update stack" : "Failed to update stacks",
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
				toastManager.add({ title, description: `Nothing to ${direction}` });
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
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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
		onSuccess: async (newRef, _input, _context, mutation) => {
			const newBranch: BranchOperand = {
				stackId,
				branchRef: newRef.fullNameBytes,
			};

			mutation.client.setQueryData(headInfoQueryOptions(projectId).queryKey, (headInfo) => {
				if (!headInfo) return headInfo;

				return renameBranchInHeadInfo({
					headInfo,
					stackId,
					branchRef,
					newName: newRef.displayName,
					newBranchRef: newRef.fullNameBytes,
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
