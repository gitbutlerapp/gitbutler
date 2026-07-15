import { decodeBytes, encodeBytes } from "#ui/api/bytes.ts";
import { getHeadInfoIndex, renameBranchInHeadInfo } from "#ui/api/ref-info.ts";
import { liteApi, queryResult } from "#ui/api/queries.ts";
import { mutationResult } from "#ui/api/mutation-result.ts";
import { shortCommitId } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import {
	discardChangesToastOptions,
	rejectedChangesToastOptions,
} from "#ui/operations/toastOptions.tsx";
import { commitOperand, type BranchOperand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { type AppDispatch, useAppDispatch } from "#ui/store.ts";
import { Toast } from "@base-ui/react";
import {
	type CommitAbsorption,
	type InsertSide,
	type RelativeTo,
	type Snapshot,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import type { OpenInEditorParams } from "#electron/ipc.ts";
import type { GUISettings } from "#electron/settings.ts";
import { moveDraftPR } from "#ui/pr.ts";

// oxlint-disable-next-line typescript/no-explicit-any
type PromiseReturnType<T> = T extends (...args: Array<any>) => Promise<infer U> ? U : never;
type AnyResponse = PromiseReturnType<(typeof window.lite)[keyof typeof window.lite]>;

export const syncCoreCaches = (
	dispatch: AppDispatch,
	projectId: string,
	response: Exclude<AnyResponse, void>,
) => {
	if (typeof response !== "object" || response === null) return;

	const workspace =
		"workspace" in response
			? response.workspace
			: "workspaceState" in response
				? response.workspaceState
				: null;
	if (workspace === null) return;

	void dispatch(liteApi.util.upsertQueryData("headInfo", projectId, workspace.headInfo));
	dispatch(
		projectSlice.actions.updateRewrittenCommitReferences({
			projectId,
			replacedCommits: workspace.replacedCommits,
			headInfo: workspace.headInfo,
		}),
	);
};

const ipcMutationApi = liteApi.injectEndpoints({
	endpoints: (builder) => ({
		ipcMutation: builder.mutation<unknown, { request: () => Promise<unknown> }>({
			queryFn: ({ request }) => queryResult(request),
		}),
	}),
});

type MutationConfig<Arg, Data> = {
	mutationFn: (input: Arg, context: { dispatch: AppDispatch }) => Promise<Data>;
	onError?: (error: unknown, input: Arg) => void | Promise<void>;
	onSuccess?: (
		data: Data,
		input: Arg,
		context: undefined,
		mutation: { dispatch: AppDispatch },
	) => void | Promise<void>;
	scope?: { id: string };
};

const useRtkMutation = <Arg, Data>(config: MutationConfig<Arg, Data>) => {
	const dispatch = useAppDispatch();
	const [trigger, state] = ipcMutationApi.useIpcMutationMutation();

	return mutationResult(
		(input: Arg) => {
			const result = trigger({ request: () => config.mutationFn(input, { dispatch }) });
			return { unwrap: () => result.unwrap() as Promise<Data> };
		},
		{ ...state, data: state.data as Data | undefined },
		{
			onError: (error, input) => config.onError?.(error, input),
			onSuccess: (data, input) => config.onSuccess?.(data, input, undefined, { dispatch }),
		},
	);
};

export const useAbsorb = ({ projectId }: { projectId: string }) => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
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

export const useApply = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.apply,
		onSuccess: async (response, input, _context, _mutation) => {
			if (response.conflictingStacks.length > 0) {
				const toastId = toastManager.add({
					type: "error",
					title: "Failed to apply branch",
					description: `'${input.existingBranch}' conflicts with existing stack in the workspace: ${response.conflictingStacks
						.map((stack) => stack.shortName)
						.join(", ")}`,
					priority: "high",
					actionProps: {
						children: "Switch to branch instead",
						onClick: () => {
							(async () => {
								const checkoutResponse = await window.lite.branchCheckout({
									projectId: input.projectId,
									branch: encodeBytes(input.existingBranch),
								});
								syncCoreCaches(dispatch, input.projectId, checkoutResponse);
								toastManager.close(toastId);
							})().catch((error) => {
								// oxlint-disable-next-line no-console
								console.error(error);

								toastManager.add({
									type: "error",
									title: "Failed to switch branch",
									description: errorMessageForToast(error),
									priority: "high",
								});
							});
						},
					},
				});
			}
		},
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

	return useRtkMutation({
		mutationFn: window.lite.branchCreate,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

export const usePublishReview = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.publishReview,
		onSuccess: async (_response, input, _context, mutation) => {
			mutation.dispatch(liteApi.util.invalidateTags([{ type: "Reviews", id: input.projectId }]));
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to create pull request",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useUpdateReview = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.updateReview,
		onSuccess: async (_response, input, _context, mutation) => {
			mutation.dispatch(
				liteApi.util.invalidateTags([
					{ type: "Reviews", id: input.projectId },
					{ type: "Review", id: `${input.projectId}:${input.reviewId}` },
				]),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to update pull request",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useSetReviewAutoMerge = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.setReviewAutoMerge,
		onError: (error, input) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: `Failed to ${input.enable ? "enable" : "disable"} pull request auto-merge`,
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useMergeReview = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.mergeReview,
		onSuccess: async (_response, input, _context, mutation) => {
			mutation.dispatch(
				liteApi.util.invalidateTags([
					{ type: "Reviews", id: input.projectId },
					{ type: "Review", id: `${input.projectId}:${input.reviewId}` },
					{ type: "ReviewMergeStatus", id: `${input.projectId}:${input.reviewId}` },
				]),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to merge pull request",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useSetReviewDraftiness = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.setReviewDraftiness,
		onSuccess: async (_response, input, _context, mutation) => {
			mutation.dispatch(
				liteApi.util.invalidateTags([
					{ type: "Reviews", id: input.projectId },
					{ type: "Review", id: `${input.projectId}:${input.reviewId}` },
					{ type: "ReviewMergeStatus", id: `${input.projectId}:${input.reviewId}` },
				]),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to update pull request",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useOpenInEditor = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
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
	const dispatch = useAppDispatch();

	return useRtkMutation({
		mutationFn: async ({ commitId }: { commitId: string }, ctx) => {
			const worktreeChanges = await ctx
				.dispatch(liteApi.endpoints.changesInWorktree.initiate(projectId, { subscribe: false }))
				.unwrap();
			const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));

			return await window.lite.commitAmend({
				projectId,
				commitId,
				changes,
				dryRun: false,
			});
		},
		onSuccess: async (response, input, _ctx, _mutation) => {
			syncCoreCaches(
				dispatch,
				projectId,
				// Workaround for https://linear.app/gitbutler/issue/GB-1570/amending-commit-has-wrong-replaced-commits
				{
					...response,
					workspace: {
						...response.workspace,
						replacedCommits: {
							...response.workspace.replacedCommits,
							...(response.newCommit !== null ? { [input.commitId]: response.newCommit } : {}),
						},
					},
				},
			);

			if (response.rejectedChanges.length > 0) {
				toastManager.add(
					rejectedChangesToastOptions({
						newCommit: response.newCommit,
						rejectedChanges: response.rejectedChanges,
					}),
				);
			}
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
	const dispatch = useAppDispatch();

	return useRtkMutation({
		mutationFn: async (
			{ message, relativeTo }: { message: string; relativeTo: RelativeTo },
			ctx,
		) => {
			const worktreeChanges = await ctx
				.dispatch(liteApi.endpoints.changesInWorktree.initiate(projectId, { subscribe: false }))
				.unwrap();
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
		onSuccess: async (response, input, _ctx, _mutation) => {
			syncCoreCaches(dispatch, projectId, response);

			if (input.relativeTo.type === "commit" && response.newCommit !== null) {
				dispatch(
					projectSlice.actions.setCommitTarget({
						projectId,
						commitTarget: { type: "commit", subject: response.newCommit },
					}),
				);
			}

			if (response.rejectedChanges.length > 0) {
				toastManager.add(
					rejectedChangesToastOptions({
						newCommit: response.newCommit,
						rejectedChanges: response.rejectedChanges,
					}),
				);
			}
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

	return useRtkMutation({
		mutationFn: window.lite.commitDiscard,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

	return useRtkMutation({
		mutationFn: window.lite.commitDiscardChanges,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

	return useRtkMutation({
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

	return useRtkMutation({
		mutationFn: window.lite.commitInsertBlank,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);

			const stackId = getHeadInfoIndex(response.workspace.headInfo).commitContextById(
				response.newCommit,
			)?.stack.id;
			if (stackId != null) {
				dispatch(
					projectSlice.actions.selectOutline({
						projectId: input.projectId,
						selection: commitOperand({ stackId, commitId: response.newCommit }),
					}),
				);
			}
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

	return useRtkMutation({
		mutationFn: window.lite.commitMove,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

	return useRtkMutation({
		mutationFn: window.lite.commitReword,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

	return useRtkMutation({
		mutationFn: window.lite.commitUncommit,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

	return useRtkMutation({
		mutationFn: window.lite.commitUncommitChanges,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

export const useWorkspaceBranchAndAncestorsPush = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.workspaceBranchAndAncestorsPush,
		onSuccess: async (_response, input, _context, mutation) => {
			mutation.dispatch(liteApi.util.invalidateTags([{ type: "HeadInfo", id: input.projectId }]));
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

export const useWorkspaceIntegrateUpstream = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.workspaceIntegrateUpstream,
		onSuccess: (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
		},
		onError: (error, input) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: input.updates.length === 1 ? "Failed to update stack" : "Failed to update stacks",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRemoveBranch = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		mutationFn: window.lite.removeBranch,
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

	return useRtkMutation({
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

	return useRtkMutation({
		mutationFn: window.lite.tearOffBranch,
		onSuccess: async (response, input, _context, _mutation) => {
			syncCoreCaches(dispatch, input.projectId, response);
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

	return useRtkMutation({
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

	return useRtkMutation({
		mutationFn: window.lite.updateBranchName,
		onSuccess: async (newRef, _input, _context, mutation) => {
			const newBranch: BranchOperand = {
				stackId,
				branchRef: newRef.fullNameBytes,
			};

			mutation.dispatch(
				liteApi.util.updateQueryData("headInfo", projectId, (headInfo) =>
					renameBranchInHeadInfo({
						headInfo,
						stackId,
						branchRef,
						newName: newRef.displayName,
						newBranchRef: newRef.fullNameBytes,
					}),
				),
			);

			dispatch(
				projectSlice.actions.updateRewrittenBranchReferences({
					projectId,
					oldBranch,
					newBranch,
				}),
			);

			await moveDraftPR({
				dispatch: mutation.dispatch,
				projectId,
				oldBranch:
					// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
					decodeBytes(oldBranch.branchRef).replace(/^refs\/heads\//, ""),
				newBranch: newRef.displayName,
			});

			dispatch(projectSlice.actions.exitMode({ projectId }));
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

/**
 * Save GUI settings mutation with partial keys. Settings are spread (shallow).
 */
// ponytail: One settings writer preserves ordering; split queues only if settings become multi-profile.
let saveGUISettingsQueue = Promise.resolve();

export const useSaveGUISettings = () => {
	const toastManager = Toast.useToastManager();

	return useRtkMutation({
		scope: { id: "gui-settings" },
		mutationFn: (cfg: Partial<GUISettings>, ctx) => {
			const save = async () => {
				// In practice we should always have some cached data at this point.
				const prev = await ctx
					.dispatch(liteApi.endpoints.getGUISettings.initiate(undefined, { subscribe: false }))
					.unwrap();
				const next: GUISettings = {
					...prev,
					...cfg,
				};

				// Update the cache immediately for UX, and keep it updated even if writing fails so that the
				// app is usable. We shan't bother invalidating the query.
				void ctx.dispatch(liteApi.util.upsertQueryData("getGUISettings", undefined, next));

				return await window.lite.writeGUISettings(next);
			};
			const result = saveGUISettingsQueue.then(save, save);
			saveGUISettingsQueue = result.then(
				() => undefined,
				() => undefined,
			);
			return result;
		},
		onError: async (err) => {
			// oxlint-disable-next-line no-console
			console.error(err);

			toastManager.add({
				type: "error",
				title: "Failed to save settings",
				description: errorMessageForToast(err),
				priority: "high",
			});
		},
	});
};
