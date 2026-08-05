import { decodeBytes, encodeBytes } from "#ui/api/bytes.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import {
	currentForgeLoginQueryOptions,
	getReviewMergeStatusQueryOptions,
	getReviewQueryOptions,
	headInfoQueryOptions,
	guiSettingsQueryOptions,
	listCommentReactionsQueryOptions,
	listReviewCommentsQueryOptions,
	listReviewReactionsQueryOptions,
	type QueryKey,
} from "#ui/api/queries.ts";
import { shortCommitId } from "#ui/commit.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import {
	discardChangesToastOptions,
	rejectedChangesToastOptions,
} from "#ui/operations/toastOptions.tsx";
import { commitOperand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { type AppDispatch, useAppDispatch } from "#ui/store.ts";
import { formatRelativeTime } from "#ui/time.ts";
import { Toast } from "@base-ui/react";
import type {
	CommitAbsorption,
	ForgeReview,
	ForgeReviewComment,
	ForgeReviewReaction,
	ForgeReviewUser,
	Snapshot,
} from "@gitbutler/but-sdk";
import { type QueryClient, useMutation } from "@tanstack/react-query";
import type { OpenInProgramParams } from "#electron/ipc.ts";
import type { GUISettings } from "#electron/settings.ts";
import { moveDraftPR } from "#ui/pr.ts";

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
		mutationFn: window.lite.branchCreate,
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

export const usePublishReview = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.publishReview,
		onSuccess: async (_response, input, _context, mutation) => {
			await mutation.client.invalidateQueries({
				queryKey: ["reviews" satisfies QueryKey, input.projectId],
			});
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

	return useMutation({
		mutationFn: window.lite.updateReview,
		onSuccess: async (_response, input, _context, mutation) => {
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: getReviewQueryOptions({ projectId: input.projectId, reviewId: input.reviewId })
						.queryKey,
				}),
			]);
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

export const useAddReviewLabels = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.addReviewLabels,
		onSuccess: async (_response, input, _context, mutation) => {
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["review" satisfies QueryKey, input.projectId],
				}),
			]);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to add label",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRemoveReviewLabel = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.removeReviewLabel,
		onSuccess: async (_response, input, _context, mutation) => {
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["review" satisfies QueryKey, input.projectId],
				}),
			]);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to remove label",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

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

export const useAddReviewReaction = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.addReviewReaction,
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
		onSettled: (_response, _err, input, _prev, ctx) =>
			ctx.client.invalidateQueries({ queryKey: listReviewReactionsQueryOptions(input).queryKey }),
		onError: (error, input, prev, ctx) => {
			if (prev) ctx.client.setQueryData(listReviewReactionsQueryOptions(input).queryKey, prev);

			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to add reaction",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRemoveReviewReaction = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.removeReviewReaction,
		onMutate: async (input, ctx) => {
			const key = listReviewReactionsQueryOptions(input).queryKey;
			await ctx.client.cancelQueries({ queryKey: key });

			const prev = ctx.client.getQueryData(key);
			ctx.client.setQueryData(key, (reactions) =>
				reactions?.filter((reaction) => reaction.id !== input.reactionId),
			);

			return prev;
		},
		onSettled: (_response, _err, input, _prev, ctx) =>
			ctx.client.invalidateQueries({ queryKey: listReviewReactionsQueryOptions(input).queryKey }),
		onError: (error, input, prev, ctx) => {
			if (prev) ctx.client.setQueryData(listReviewReactionsQueryOptions(input).queryKey, prev);

			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to remove reaction",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

/**
 * A comment reaction spans two caches — the count summary on the comments
 * listing and the names on the per-comment reactions listing — so the
 * optimistic write and its rollback patch both.
 */
export const useAddCommentReaction = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.addCommentReaction,
		onMutate: async (input, ctx) => {
			const reactionsKey = listCommentReactionsQueryOptions(input).queryKey;
			const commentsKey = listReviewCommentsQueryOptions(input).queryKey;
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
		onSettled: (_response, _err, input, _prev, ctx) =>
			Promise.all([
				ctx.client.invalidateQueries({
					queryKey: listCommentReactionsQueryOptions(input).queryKey,
				}),
				ctx.client.invalidateQueries({ queryKey: listReviewCommentsQueryOptions(input).queryKey }),
			]),
		onError: (error, input, prev, ctx) => {
			if (prev?.prevReactions) {
				ctx.client.setQueryData(
					listCommentReactionsQueryOptions(input).queryKey,
					prev.prevReactions,
				);
			}
			if (prev?.prevComments)
				ctx.client.setQueryData(listReviewCommentsQueryOptions(input).queryKey, prev.prevComments);

			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to add reaction",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRemoveCommentReaction = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.removeCommentReaction,
		onMutate: async (input, ctx) => {
			const reactionsKey = listCommentReactionsQueryOptions(input).queryKey;
			const commentsKey = listReviewCommentsQueryOptions(input).queryKey;
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
		onSettled: (_response, _err, input, _prev, ctx) =>
			Promise.all([
				ctx.client.invalidateQueries({
					queryKey: listCommentReactionsQueryOptions(input).queryKey,
				}),
				ctx.client.invalidateQueries({ queryKey: listReviewCommentsQueryOptions(input).queryKey }),
			]),
		onError: (error, input, prev, ctx) => {
			if (prev?.prevReactions) {
				ctx.client.setQueryData(
					listCommentReactionsQueryOptions(input).queryKey,
					prev.prevReactions,
				);
			}
			if (prev?.prevComments)
				ctx.client.setQueryData(listReviewCommentsQueryOptions(input).queryKey, prev.prevComments);

			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to remove reaction",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useRequestReview = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.requestReview,
		onSuccess: async (_response, input, _context, mutation) => {
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["review" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["reviewTimelineEvents" satisfies QueryKey, input.projectId],
				}),
			]);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to request review",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useWithdrawReviewRequest = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.withdrawReviewRequest,
		onSuccess: async (_response, input, _context, mutation) => {
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["review" satisfies QueryKey, input.projectId],
				}),
			]);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to withdraw review request",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCreateReviewComment = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.createReviewComment,
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
		onSettled: (_response, _err, input, _prev, ctx) =>
			ctx.client.invalidateQueries({ queryKey: listReviewCommentsQueryOptions(input).queryKey }),
		onError: (error, input, prev, ctx) => {
			if (prev) ctx.client.setQueryData(listReviewCommentsQueryOptions(input).queryKey, prev);

			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to post comment",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useUpdateReviewComment = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.updateReviewComment,
		onSuccess: async (_response, input, _context, mutation) => {
			await mutation.client.invalidateQueries({
				queryKey: ["reviewComments" satisfies QueryKey, input.projectId, input.reviewId],
			});
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to update comment",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useDeleteReviewComment = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.deleteReviewComment,
		onSuccess: async (_response, input, _context, mutation) => {
			await mutation.client.invalidateQueries({
				queryKey: ["reviewComments" satisfies QueryKey, input.projectId, input.reviewId],
			});
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to delete comment",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useSetReviewAutoMerge = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.setReviewAutoMerge,
		onMutate: async (input, ctx) => {
			const reviewsPrefix = ["reviews" satisfies QueryKey, input.projectId];
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
		onSettled: (_response, _err, input, _prev, ctx) =>
			Promise.all([
				ctx.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				ctx.client.invalidateQueries({
					queryKey: ["review" satisfies QueryKey, input.projectId],
				}),
			]),
		onError: (error, input, prev, ctx) => {
			for (const [key, data] of prev?.prev ?? []) ctx.client.setQueryData(key, data);
			if (prev?.prevSingle) {
				ctx.client.setQueryData(
					getReviewQueryOptions({ projectId: input.projectId, reviewId: input.reviewId }).queryKey,
					prev.prevSingle,
				);
			}

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

	return useMutation({
		mutationFn: window.lite.mergeReview,
		onSuccess: async (_response, input, _context, mutation) => {
			// Checks 422 once the branch is merged; refetch so the badge clears.
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["review" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["reviewMergeStatus" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["ciChecks" satisfies QueryKey, input.projectId],
				}),
			]);
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

	return useMutation({
		mutationFn: window.lite.setReviewDraftiness,
		onSuccess: async (_response, input, _context, mutation) => {
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: getReviewQueryOptions({ projectId: input.projectId, reviewId: input.reviewId })
						.queryKey,
				}),
				mutation.client.invalidateQueries({
					queryKey: getReviewMergeStatusQueryOptions({
						projectId: input.projectId,
						reviewId: input.reviewId,
					}).queryKey,
				}),
			]);
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

export const useOpenInProgram = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (input: OpenInProgramParams) => window.lite.openInProgram(input),
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

export const commitAmendMutationKey = ["commitAmend"];
export const useCommitAmend = () => {
	const toastManager = Toast.useToastManager();
	const dispatch = useAppDispatch();

	return useMutation({
		mutationKey: commitAmendMutationKey,
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

export const useCommitUncommit = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitUncommit,
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

export const useWorkspaceBranchAndAncestorsPush = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.workspaceBranchAndAncestorsPush,
		onSuccess: async (_response, input, _context, mutation) => {
			// A push moves the review's head, so the cached reviews, their mergeability,
			// and the checks for the new sha are all stale.
			await Promise.all([
				mutation.client.invalidateQueries({
					queryKey: ["headInfo" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["reviews" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["review" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["reviewMergeStatus" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["ciChecks" satisfies QueryKey, input.projectId],
				}),
				mutation.client.invalidateQueries({
					queryKey: ["reviewTimelineEvents" satisfies QueryKey, input.projectId],
				}),
			]);
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

	return useMutation({
		mutationFn: window.lite.workspaceIntegrateUpstream,
		onSuccess: (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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

export const useBranchRemove = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.branchRemove,
		onSuccess: (response, input, _context, mutation) => {
			syncCoreCaches(mutation.client, dispatch, input.projectId, response);
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

			const relativeTime = formatRelativeTime(snapshot.createdAt);

			toastManager.add({
				type: "info",
				title,
				description: `Restored to ${shortCommitId(snapshot.commitId)} (${op !== undefined ? `${op}, ` : ""}${relativeTime})`,
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

export const useBranchRename = () => {
	const dispatch = useAppDispatch();
	const toastManager = Toast.useToastManager();

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
export const useSaveGUISettings = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
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
