import { decodeBytes, encodeBytes } from "#ui/api/bytes.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import {
	currentForgeLoginQueryOptions,
	getReviewQueryOptions,
	headInfoQueryOptions,
	guiSettingsQueryOptions,
	listCommentReactionsQueryOptions,
	listReviewCommentsQueryOptions,
	listReviewReactionsQueryOptions,
	workspaceFetchQueryOptions,
} from "#ui/api/queries.ts";
import { shortCommitId } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { createDiffSpec, resolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import {
	discardChangesToastOptions,
	rejectedChangesToastOptions,
} from "#ui/operations/toastOptions.tsx";
import { commitOperand, filesUnder, type FileParent } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { type AppDispatch, useAppDispatch, useAppStore } from "#ui/store.ts";
import { formatRelativeTime } from "#ui/time.ts";
import { Toast } from "@base-ui/react";
import { Match } from "effect";
import type {
	CommitAbsorption,
	DiffSpec,
	ForgeReview,
	ForgeReviewComment,
	ForgeReviewReaction,
	ForgeReviewUser,
	Snapshot,
	TreeChange,
} from "@gitbutler/but-sdk";
import { type QueryClient, useMutation, useQueryClient } from "@tanstack/react-query";
import type { GUISettings } from "#electron/settings.ts";
import { moveDraftPR } from "#ui/pr.ts";

declare module "@tanstack/react-query" {
	interface Register {
		/**
		 * A mutation's failure toast is declared, not coded: the mutation cache
		 * logs every error and shows `failureTitle` when one is given. Hooks
		 * write `onError` only for work of their own, like rolling back an
		 * optimistic write or wording a title dynamically.
		 */
		mutationMeta: { failureTitle?: string };
	}
}

const pluralRules = new Intl.PluralRules("en");

// oxlint-disable-next-line typescript/no-explicit-any
type PromiseReturnType<T> = T extends (...args: Array<any>) => Promise<infer U> ? U : never;
type AnyResponse = PromiseReturnType<(typeof window.lite)[keyof typeof window.lite]>;

export const syncCoreCaches = (
	queryClient: QueryClient,
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

	queryClient.setQueryData(headInfoQueryOptions(projectId).queryKey, workspace.headInfo);
	dispatch(
		projectSlice.actions.updateRewrittenCommitReferences({
			projectId,
			replacedCommits: workspace.replacedCommits,
		}),
	);
};

export const useAbsorb = ({ projectId }: { projectId: string }) =>
	useMutation({
		mutationFn: (absorptionPlan: Array<CommitAbsorption> | undefined) => {
			if (!absorptionPlan) return Promise.resolve(null);
			return window.lite.absorb({ projectId, absorptionPlan });
		},
		meta: { failureTitle: "Failed to absorb" },
	});

export const useApply = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.apply,
		onSuccess: async (response, input, _context, mutation) => {
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
								syncCoreCaches(mutation.client, dispatch, input.projectId, checkoutResponse);
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
		meta: { failureTitle: "Failed to apply branch" },
	});
};

export const useBranchCreate = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.branchCreate,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to create branch" },
	});
};

export const usePublishReview = () =>
	useMutation({
		mutationFn: window.lite.publishReview,
		meta: { failureTitle: "Failed to create pull request" },
	});

export const useUpdateReview = () =>
	useMutation({
		mutationFn: window.lite.updateReview,
		meta: { failureTitle: "Failed to update pull request" },
	});

export const useAddReviewLabels = () =>
	useMutation({
		mutationFn: window.lite.addReviewLabels,
		meta: { failureTitle: "Failed to add label" },
	});

export const useRemoveReviewLabel = () =>
	useMutation({
		mutationFn: window.lite.removeReviewLabel,
		meta: { failureTitle: "Failed to remove label" },
	});

/**
 * Optimistic entries carry negative forge ids until the settle refetch
 * swaps in the real ones; the UI treats them as not-yet-actionable.
 * Each ghost takes a fresh id so simultaneous ghosts keep distinct
 * render keys.
 */
let nextOptimisticForgeId = -1;
const takeOptimisticForgeId = () => nextOptimisticForgeId--;

const ghostForgeUser = (login: string): ForgeReviewUser => ({
	id: takeOptimisticForgeId(),
	login,
	name: null,
	email: null,
	avatarUrl: null,
	isBot: false,
});

const ghostReaction = (kind: string, login: string): ForgeReviewReaction => ({
	id: takeOptimisticForgeId(),
	kind,
	user: ghostForgeUser(login),
});

/** Bump one kind's tally on one comment; entries appear and vanish at zero. */
const withCommentReactionCount = (
	comments: Array<ForgeReviewComment> | undefined,
	commentId: number,
	kind: string,
	delta: number,
): Array<ForgeReviewComment> | undefined =>
	comments?.map((comment) => {
		if (comment.id !== commentId) return comment;
		const existing = comment.reactions.find((entry) => entry.kind === kind);
		const next = (existing?.count ?? 0) + delta;
		const reactions =
			existing === undefined
				? next > 0
					? [...comment.reactions, { kind, count: next }]
					: comment.reactions
				: next > 0
					? comment.reactions.map((entry) =>
							entry.kind === kind ? { ...entry, count: next } : entry,
						)
					: comment.reactions.filter((entry) => entry.kind !== kind);
		return { ...comment, reactions };
	});

export const useAddReviewReaction = () =>
	useMutation({
		mutationFn: window.lite.addReviewReaction,
		meta: { failureTitle: "Failed to add reaction" },
		onMutate: async (input, ctx) => {
			const key = listReviewReactionsQueryOptions(input).queryKey;
			await ctx.client.cancelQueries({ queryKey: key });

			const prev = ctx.client.getQueryData(key);
			const login = ctx.client.getQueryData(
				currentForgeLoginQueryOptions(input.projectId).queryKey,
			);
			if (login != null) {
				ctx.client.setQueryData(key, (reactions) =>
					(reactions ?? []).concat(ghostReaction(input.kind, login)),
				);
			}

			return prev;
		},
		onError: (error, input, prev, ctx) => {
			// Roll the optimistic write back, then refetch: the rollback snapshot
			// may itself be stale by now.
			if (prev) ctx.client.setQueryData(listReviewReactionsQueryOptions(input).queryKey, prev);
			void ctx.client.invalidateQueries({
				queryKey: listReviewReactionsQueryOptions(input).queryKey,
			});
		},
	});

export const useRemoveReviewReaction = () =>
	useMutation({
		mutationFn: window.lite.removeReviewReaction,
		meta: { failureTitle: "Failed to remove reaction" },
		onMutate: async (input, ctx) => {
			const key = listReviewReactionsQueryOptions(input).queryKey;
			await ctx.client.cancelQueries({ queryKey: key });

			const prev = ctx.client.getQueryData(key);
			ctx.client.setQueryData(key, (reactions) =>
				reactions?.filter((reaction) => reaction.id !== input.reactionId),
			);

			return prev;
		},
		onError: (error, input, prev, ctx) => {
			// Roll the optimistic write back, then refetch: the rollback snapshot
			// may itself be stale by now.
			if (prev) ctx.client.setQueryData(listReviewReactionsQueryOptions(input).queryKey, prev);
			void ctx.client.invalidateQueries({
				queryKey: listReviewReactionsQueryOptions(input).queryKey,
			});
		},
	});

/**
 * A comment reaction spans two caches — the count summary on the comments
 * listing and the names on the per-comment reactions listing — so the
 * optimistic write and its rollback patch both.
 */
export const useAddCommentReaction = ({ reviewId }: { reviewId: number }) =>
	useMutation({
		mutationFn: window.lite.addCommentReaction,
		meta: { failureTitle: "Failed to add reaction" },
		onMutate: async (input, ctx) => {
			const reactionsKey = listCommentReactionsQueryOptions(input).queryKey;
			const commentsKey = listReviewCommentsQueryOptions({
				projectId: input.projectId,
				reviewId,
			}).queryKey;
			await Promise.all([
				ctx.client.cancelQueries({ queryKey: reactionsKey }),
				ctx.client.cancelQueries({ queryKey: commentsKey }),
			]);

			const prevReactions = ctx.client.getQueryData(reactionsKey);
			const prevComments = ctx.client.getQueryData(commentsKey);
			const login = ctx.client.getQueryData(
				currentForgeLoginQueryOptions(input.projectId).queryKey,
			);
			if (login != null) {
				ctx.client.setQueryData(reactionsKey, (reactions) =>
					(reactions ?? []).concat(ghostReaction(input.kind, login)),
				);
				ctx.client.setQueryData(commentsKey, (comments) =>
					withCommentReactionCount(comments, input.commentId, input.kind, 1),
				);
			}

			return { prevReactions, prevComments };
		},
		onError: (error, input, prev, ctx) => {
			const reactionsKey = listCommentReactionsQueryOptions(input).queryKey;
			const commentsKey = listReviewCommentsQueryOptions({
				projectId: input.projectId,
				reviewId,
			}).queryKey;
			// Roll the optimistic writes back, then refetch: the rollback
			// snapshots may themselves be stale by now.
			if (prev?.prevReactions) ctx.client.setQueryData(reactionsKey, prev.prevReactions);
			if (prev?.prevComments) ctx.client.setQueryData(commentsKey, prev.prevComments);
			void ctx.client.invalidateQueries({ queryKey: reactionsKey });
			void ctx.client.invalidateQueries({ queryKey: commentsKey });
		},
	});

export const useRemoveCommentReaction = ({ reviewId }: { reviewId: number }) =>
	useMutation({
		mutationFn: window.lite.removeCommentReaction,
		meta: { failureTitle: "Failed to remove reaction" },
		onMutate: async (input, ctx) => {
			const reactionsKey = listCommentReactionsQueryOptions(input).queryKey;
			const commentsKey = listReviewCommentsQueryOptions({
				projectId: input.projectId,
				reviewId,
			}).queryKey;
			await Promise.all([
				ctx.client.cancelQueries({ queryKey: reactionsKey }),
				ctx.client.cancelQueries({ queryKey: commentsKey }),
			]);

			const prevReactions = ctx.client.getQueryData(reactionsKey);
			const prevComments = ctx.client.getQueryData(commentsKey);
			// The removed reaction's kind drives the count patch; it's in the
			// listing the toggle was derived from.
			const kind = prevReactions?.find((reaction) => reaction.id === input.reactionId)?.kind;
			ctx.client.setQueryData(reactionsKey, (reactions) =>
				reactions?.filter((reaction) => reaction.id !== input.reactionId),
			);
			if (kind !== undefined) {
				ctx.client.setQueryData(commentsKey, (comments) =>
					withCommentReactionCount(comments, input.commentId, kind, -1),
				);
			}

			return { prevReactions, prevComments };
		},
		onError: (error, input, prev, ctx) => {
			const reactionsKey = listCommentReactionsQueryOptions(input).queryKey;
			const commentsKey = listReviewCommentsQueryOptions({
				projectId: input.projectId,
				reviewId,
			}).queryKey;
			// Roll the optimistic writes back, then refetch: the rollback
			// snapshots may themselves be stale by now.
			if (prev?.prevReactions) ctx.client.setQueryData(reactionsKey, prev.prevReactions);
			if (prev?.prevComments) ctx.client.setQueryData(commentsKey, prev.prevComments);
			void ctx.client.invalidateQueries({ queryKey: reactionsKey });
			void ctx.client.invalidateQueries({ queryKey: commentsKey });
		},
	});

export const useRequestReview = () =>
	useMutation({
		mutationFn: window.lite.requestReview,
		meta: { failureTitle: "Failed to request review" },
	});

export const useWithdrawReviewRequest = () =>
	useMutation({
		mutationFn: window.lite.withdrawReviewRequest,
		meta: { failureTitle: "Failed to withdraw review request" },
	});

export const useCreateReviewComment = () =>
	useMutation({
		mutationFn: window.lite.createReviewComment,
		meta: { failureTitle: "Failed to post comment" },
		onMutate: async (input, ctx) => {
			const key = listReviewCommentsQueryOptions(input).queryKey;
			await ctx.client.cancelQueries({ queryKey: key });

			const prev = ctx.client.getQueryData(key);
			const login = ctx.client.getQueryData(
				currentForgeLoginQueryOptions(input.projectId).queryKey,
			);
			const ghost: ForgeReviewComment = {
				id: takeOptimisticForgeId(),
				body: input.body,
				author: login == null ? null : ghostForgeUser(login),
				createdAt: new Date().toISOString(),
				modifiedAt: null,
				htmlUrl: "",
				reactions: [],
			};
			ctx.client.setQueryData(key, (comments) => (comments ?? []).concat(ghost));

			return prev;
		},
		onError: (error, input, prev, ctx) => {
			// Roll the optimistic write back, then refetch: the rollback snapshot
			// may itself be stale by now.
			if (prev) ctx.client.setQueryData(listReviewCommentsQueryOptions(input).queryKey, prev);
			void ctx.client.invalidateQueries({
				queryKey: listReviewCommentsQueryOptions(input).queryKey,
			});
		},
	});

export const useUpdateReviewComment = () =>
	useMutation({
		mutationFn: window.lite.updateReviewComment,
		meta: { failureTitle: "Failed to update comment" },
	});

export const useDeleteReviewComment = () =>
	useMutation({
		mutationFn: window.lite.deleteReviewComment,
		meta: { failureTitle: "Failed to delete comment" },
	});

export const useSetReviewAutoMerge = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.setReviewAutoMerge,
		onMutate: async (input, ctx) => {
			const reviewsPrefix = ["listReviews", input.projectId] as const;
			await ctx.client.cancelQueries({ queryKey: reviewsPrefix });

			// The flag lives on every reviews listing (the key varies by cache
			// config) plus the single-review cache; patch them all, snapshot
			// for rollback.
			const prev = ctx.client.getQueriesData<Array<ForgeReview>>({ queryKey: reviewsPrefix });
			ctx.client.setQueriesData<Array<ForgeReview>>({ queryKey: reviewsPrefix }, (reviews) =>
				reviews?.map((review) =>
					review.number === input.reviewId ? { ...review, autoMergeEnabled: input.enable } : review,
				),
			);
			const singleKey = getReviewQueryOptions({
				projectId: input.projectId,
				reviewId: input.reviewId,
			}).queryKey;
			const prevSingle = ctx.client.getQueryData(singleKey);
			ctx.client.setQueryData(singleKey, (review) =>
				review === undefined ? undefined : { ...review, autoMergeEnabled: input.enable },
			);

			return { prev, prevSingle };
		},
		onError: (error, input, prev, ctx) => {
			// Roll the optimistic writes back, then refetch: the rollback
			// snapshots may themselves be stale by now.
			for (const [key, data] of prev?.prev ?? []) ctx.client.setQueryData(key, data);
			if (prev?.prevSingle) {
				ctx.client.setQueryData(
					getReviewQueryOptions({ projectId: input.projectId, reviewId: input.reviewId }).queryKey,
					prev.prevSingle,
				);
			}
			void ctx.client.invalidateQueries({ queryKey: ["listReviews", input.projectId] });
			void ctx.client.invalidateQueries({ queryKey: ["getReview", input.projectId] });

			toastManager.add({
				type: "error",
				title: `Failed to ${input.enable ? "enable" : "disable"} pull request auto-merge`,
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useMergeReview = () =>
	useMutation({
		mutationFn: window.lite.mergeReview,
		onSuccess: async (_response, input, _context, mutation) => {
			// The merge moved the target branch on the remote, but nothing local, so
			// the branch keeps looking un-integrated until remote-tracking refs catch
			// up. Fetch through the shared query (dedupes with auto-fetch), then
			// re-read head info rather than waiting on watcher delivery. A failed
			// fetch is not a failed merge, so neither reaches onError.
			await mutation.client
				.fetchQuery({ ...workspaceFetchQueryOptions(input.projectId), staleTime: 0 })
				.then(() =>
					mutation.client.fetchQuery({
						...headInfoQueryOptions(input.projectId),
						staleTime: 0,
					}),
				)
				.catch(() => undefined);
		},
		meta: { failureTitle: "Failed to merge pull request" },
	});

export const useSetReviewDraftiness = () =>
	useMutation({
		mutationFn: window.lite.setReviewDraftiness,
		meta: { failureTitle: "Failed to update pull request" },
	});

export const useSetGbConfig = () =>
	useMutation({
		mutationFn: window.lite.setGbConfig,
		meta: { failureTitle: "Failed to save git settings" },
	});

export const useDeleteAllData = () =>
	useMutation({
		mutationFn: window.lite.deleteAllData,
		meta: { failureTitle: "Failed to remove projects" },
	});

export const useForgetGithubAccount = () =>
	useMutation({
		mutationFn: window.lite.forgetGithubAccount,
		meta: { failureTitle: "Failed to forget account" },
	});

export const useForgetGitlabAccount = () =>
	useMutation({
		mutationFn: window.lite.forgetGitlabAccount,
		meta: { failureTitle: "Failed to forget account" },
	});

export const useForgetBitbucketAccount = () =>
	useMutation({
		mutationFn: window.lite.forgetBitbucketAccount,
		meta: { failureTitle: "Failed to forget account" },
	});

export const useStoreGithubPat = () =>
	useMutation({
		mutationFn: window.lite.storeGithubPat,
		meta: { failureTitle: "Failed to add GitHub account" },
	});

export const useStoreGitlabPat = () =>
	useMutation({
		mutationFn: window.lite.storeGitlabPat,
		meta: { failureTitle: "Failed to add GitLab account" },
	});

export const useStoreBitbucketApiToken = () =>
	useMutation({
		mutationFn: window.lite.storeBitbucketApiToken,
		meta: { failureTitle: "Failed to add Bitbucket account" },
	});

export const useDeleteProject = () =>
	useMutation({
		mutationFn: window.lite.deleteProject,
		meta: { failureTitle: "Failed to remove project" },
	});

export const useUpdateProjectSettings = () =>
	useMutation({
		mutationFn: window.lite.updateProjectSettings,
		meta: { failureTitle: "Failed to save project settings" },
	});

export const useOpenInProgram = () =>
	useMutation({
		mutationFn: window.lite.openInProgram,
		meta: { failureTitle: "Failed to open in editor" },
	});

export const useCommitAmend = () => {
	const toastManager = Toast.useToastManager();
	const dispatch = useAppDispatch();

	return useMutation({
		mutationFn: window.lite.commitAmend,
		onSuccess: async (response, input, _ctx, mutation) => {
			syncCoreCaches(
				mutation.client,
				dispatch,
				input.projectId,
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
		meta: { failureTitle: "Failed to amend commit" },
	});
};

export const useCommitCreate = () => {
	const toastManager = Toast.useToastManager();
	const dispatch = useAppDispatch();

	return useMutation({
		mutationFn: window.lite.commitCreate,
		onSuccess: async (response, input, _ctx, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);

			if (input.relativeTo.type === "commit" && response.newCommit !== null) {
				const headInfoIndex = getHeadInfoIndex(response.workspace.headInfo);
				const newCommitCtx = headInfoIndex.commitContextByCommitId(response.newCommit);

				if (newCommitCtx) {
					dispatch(
						projectSlice.actions.selectOutline({
							projectId: input.projectId,
							selection: commitOperand({
								commitId: response.newCommit,
								changeId: newCommitCtx.commit.changeId,
							}),
						}),
					);
				}
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
		meta: { failureTitle: "Failed to commit" },
	});
};

export const useCommitDiscard = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.commitDiscard,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to discard commit" },
	});
};

export const useCommitDiscardChanges = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.commitDiscardChanges,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to discard changes" },
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
		meta: { failureTitle: "Failed to discard changes" },
	});
};

/** Discards a file's changes, whichever of the two discards its parent calls for. */
export const useDiscardFileChanges = ({
	projectId,
	fileParent,
}: {
	projectId: string;
	fileParent: FileParent;
}) => {
	const store = useAppStore();
	const queryClient = useQueryClient();
	const toastManager = Toast.useToastManager();
	const { isPending: isCommitDiscardChangesPending, mutate: commitDiscardChanges } =
		useCommitDiscardChanges();
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();

	const canDiscard = Match.value(fileParent).pipe(
		Match.tagsExhaustive({
			Commit: () => !isCommitDiscardChangesPending,
			UncommittedChanges: () => !isDiscardWorktreeChangesPending,
			// We currently don't support any operations on branch files.
			Branch: () => false,
		}),
	);

	const runDiscard = (changes: Array<DiffSpec>): void =>
		Match.value(fileParent).pipe(
			Match.tagsExhaustive({
				Commit: ({ commitId }) => {
					commitDiscardChanges({ projectId, commitId, changes, dryRun: false });
				},
				UncommittedChanges: () => {
					discardWorktreeChanges({ projectId, worktreeChanges: changes });
				},
				Branch: () => {},
			}),
		);

	/**
	 * Discard `change`, extended to the checked files when `extendToCheckedFiles` — a row's menu
	 * passes its own checked state, as dragging does; a list hotkey passes true, as cut and move do.
	 */
	const discard = async ({
		change,
		extendToCheckedFiles,
	}: {
		change: TreeChange;
		extendToCheckedFiles: boolean;
	}): Promise<void> => {
		// A checked set belonging to another list says nothing about this file, so the subject then
		// stands alone, as it does when nothing is checked at all.
		const checkedFiles = extendToCheckedFiles
			? filesUnder(
					projectSlice.selectors.selectCheckedOperands(store.getState(), projectId),
					fileParent,
				)
			: [];
		if (checkedFiles.length === 0) return runDiscard([createDiffSpec(change, [])]);

		// Checked files carry only paths, so their changes have to be looked up.
		try {
			const changes = await resolveDiffSpecs({ projectId, queryClient, sources: checkedFiles });
			// One of them gone stale fails resolution for the whole set — the reconciler is about to
			// uncheck it — and discarding the subject instead is not what was asked for.
			if (changes) runDiscard(changes);
		} catch (error) {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to discard changes",
				description: errorMessageForToast(error),
				priority: "high",
			});
		}
	};

	return { canDiscard, discard };
};

export const useCommitInsertBlank = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.commitInsertBlank,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);

			const headInfoIndex = getHeadInfoIndex(response.workspace.headInfo);
			const newCommitCtx = headInfoIndex.commitContextByCommitId(response.newCommit);

			if (newCommitCtx) {
				dispatch(
					projectSlice.actions.selectOutline({
						projectId: input.projectId,
						selection: commitOperand({
							commitId: response.newCommit,
							changeId: newCommitCtx.commit.changeId,
						}),
					}),
				);
			}
		},
		meta: { failureTitle: "Failed to insert commit" },
	});
};

export const useCommitMove = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.commitMove,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to move commit" },
	});
};

export const useCommitReword = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.commitReword,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to reword commit" },
	});
};

/**
 * Resolve some of a conflicted commit's conflicts. Every apply rewrites the
 * commit, so the reply carries the replaced ids that `syncCoreCaches` feeds to
 * the store — selection and checked operands follow the new commit on their own,
 * and the conflicts query re-reads under the new id.
 */
export const useResolveCommitConflictHunks = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.resolveCommitConflictHunks,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
			// No check clearing: ids survive the rewrite, resolved ones stop matching.

			// A commit with a manual-only file left is still conflicted, however
			// many hunks were resolved, so both lists must be empty to be done.
			if (response.remaining.length === 0 && response.manual.length === 0) {
				toastManager.add({
					type: "success",
					title: "All conflicts resolved",
					description: response.commitEmptied
						? "The commit keeps nothing of its own now, so it no longer changes anything. Undo from the operations history if that wasn't the intent."
						: "The commit is no longer conflicted.",
					priority: "low",
				});
			} else {
				const remaining = response.remaining.reduce((sum, file) => sum + file.hunks, 0);
				toastManager.add({
					type: "success",
					title:
						response.resolved === 1
							? "Conflict resolved"
							: `${response.resolved} conflicts resolved`,
					description:
						remaining > 0
							? `${remaining} conflict${remaining === 1 ? "" : "s"} remaining in this commit.`
							: "The remaining files can only be resolved in edit mode.",
					priority: "low",
				});
			}
		},
		meta: { failureTitle: "Failed to resolve the conflict" },
	});
};

export const useCommitUncommit = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.commitUncommit,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to uncommit" },
	});
};

export const useCommitUncommitChanges = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.commitUncommitChanges,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to uncommit" },
	});
};

export const useWorkspaceBranchAndAncestorsPush = () =>
	useMutation({
		mutationFn: window.lite.workspaceBranchAndAncestorsPush,
		meta: { failureTitle: "Failed to push" },
	});

export const useWorkspaceIntegrateUpstream = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.workspaceIntegrateUpstream,
		onSuccess: (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		onError: (error, input) => {
			toastManager.add({
				type: "error",
				title: `Failed to update stack${pluralRules.select(input.updates.length) === "one" ? "" : "s"}`,
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useBranchRemove = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.branchRemove,
		onSuccess: (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to delete branch reference" },
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

			const relativeTime = formatRelativeTime(snapshot.createdAt);

			toastManager.add({
				type: "info",
				title,
				description: `Restored to ${shortCommitId(snapshot.commitId)} (${op !== undefined ? `${op}, ` : ""}${relativeTime})`,
			});
		},
		onError: (error, direction) => {
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
	return useMutation({
		mutationFn: window.lite.tearOffBranch,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
		},
		meta: { failureTitle: "Failed to tear off branch" },
	});
};

export const useUnapplyStack = () =>
	useMutation({
		mutationFn: window.lite.unapplyStack,
		meta: { failureTitle: "Failed to unapply stack" },
	});

export const useBranchRename = () => {
	const dispatch = useAppDispatch();
	return useMutation({
		mutationFn: window.lite.branchRename,
		onSuccess: async (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);

			dispatch(
				projectSlice.actions.updateRewrittenBranchReferences({
					projectId: input.projectId,
					oldBranch: {
						branchRef: input.refName,
					},
					newBranch: {
						branchRef: response.newRef.fullNameBytes,
					},
				}),
			);

			await moveDraftPR({
				queryClient: mutation.client,
				projectId: input.projectId,
				oldBranch:
					// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
					decodeBytes(input.refName).replace(/^refs\/heads\//, ""),
				newBranch: response.newRef.displayName,
			});

			dispatch(projectSlice.actions.exitMode({ projectId: input.projectId }));
		},
		meta: { failureTitle: "Failed to rename branch" },
	});
};

/**
 * Save GUI settings mutation with partial keys. Settings are spread (shallow).
 */
export const useSaveGUISettings = () =>
	useMutation({
		scope: { id: "guiSettings" },
		mutationFn: async (cfg: Partial<GUISettings>, ctx) => {
			// In practice we should always have some cached data at this point.
			const prev = await ctx.client.ensureQueryData(guiSettingsQueryOptions);
			const next: GUISettings = {
				...prev,
				...cfg,
			};

			// Update the cache immediately for UX, and keep it updated even if writing fails so that the
			// app is usable. We shan't bother invalidating the query.
			ctx.client.setQueryData(guiSettingsQueryOptions.queryKey, next);

			return await window.lite.writeGUISettings(next);
		},
		meta: { failureTitle: "Failed to save settings" },
	});
