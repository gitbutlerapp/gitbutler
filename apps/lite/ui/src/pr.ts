import { forgeInfoOptions, getReviewQueryOptions } from "#ui/api/queries.ts";
import type { ForgeInfo, ReviewMergeMethod } from "@gitbutler/but-sdk";
import { type QueryClient, queryOptions, useMutation, useQuery } from "@tanstack/react-query";
import * as idb from "idb-keyval";

export const prForgeUrl = (prNo: number, forge: ForgeInfo): string =>
	`${forge.baseUrl}${forge.prUrlPath}${prNo}`;

/**
 * Verify that a persisted review number identifies a merged review: the
 * stored number alone cannot say what became of its review, so the review is
 * fetched and the number is returned until the fetch proves it did NOT merge.
 * Optimistic on purpose — while loading, on a failed fetch, and offline,
 * suppressing the create-PR flow beats flashing it for a landed branch, and
 * it matches the branch row's chip. Null only for a review verified as
 * closed-without-merging, and when no number was given. `enabled` defers the
 * fetch until the consumer actually needs the verdict; the returned value
 * stays live either way, so read it only where review state is shown.
 */
export const useLandedReviewId = (
	projectId: string,
	reviewId: number | null,
	enabled: boolean,
): number | null => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: isMerged } = useQuery({
		...getReviewQueryOptions({ projectId, reviewId: reviewId ?? 0 }),
		enabled: enabled && reviewId !== null && forgeInfo?.capabilities.prService === true,
		select: (review) => review.mergedAt !== null,
	});
	if (reviewId === null) return null;
	return isMerged === false ? null : reviewId;
};

const mergeMethodKey = (projectId: string): string => `pr_merge_method:v1:${projectId}`;

export const mergeMethodQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["prMergeMethod", projectId],
		queryFn: async () =>
			(await idb.get<ReviewMergeMethod>(mergeMethodKey(projectId))) ?? ("merge" as const),
	});

export const usePersistMergeMethod = () =>
	useMutation({
		mutationFn: ({ projectId, method }: { projectId: string; method: ReviewMergeMethod }) =>
			idb.set(mergeMethodKey(projectId), method),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(mergeMethodQueryOptions(input.projectId).queryKey, input.method),
	});

/**
 * All fields optional so a record written by an older build still reads.
 *
 * `labels` and `reviewers` are settings for a PR that does not exist yet: the
 * forge takes neither when creating one, so they are held here until there is
 * a review to apply them to.
 */
type DraftPR = {
	title?: string;
	body?: string;
	isDraft?: boolean;
	labels?: Array<string>;
	reviewers?: Array<string>;
};

/** The part of a draft the panel beside the create form owns. */
export type DraftPRExtras = {
	labels: Array<string>;
	reviewers: Array<string>;
};

// Branch name isn't stable identity. Ideally in the future this'd be written to Git metadata.
const draftPRKey = ({ projectId, branchName }: { projectId: string; branchName: string }): string =>
	`pr_draft:v1:${projectId}:${branchName}`;

/** Move a draft PR, if any, from an old branch name to a new one following a rename. */
export const moveDraftPR = async ({
	queryClient,
	projectId,
	oldBranch,
	newBranch,
}: {
	queryClient: QueryClient;
	projectId: string;
	oldBranch: string;
	newBranch: string;
}): Promise<void> => {
	const prevKey = draftPRKey({ projectId, branchName: oldBranch });
	const draft = await idb.get<DraftPR>(prevKey);
	if (!draft) return;

	const newKey = draftPRKey({ projectId, branchName: newBranch });
	await idb.set(newKey, draft);
	queryClient.setQueryData(
		draftPRQueryOptions({ projectId, branchName: newBranch }).queryKey,
		draft,
	);

	await idb.del(prevKey);
	queryClient.removeQueries({
		queryKey: draftPRQueryOptions({ projectId, branchName: oldBranch }).queryKey,
	});
};

export const draftPRQueryOptions = ({
	projectId,
	branchName,
}: {
	projectId: string;
	branchName: string;
}) =>
	queryOptions({
		queryKey: ["prDraft", projectId, branchName],
		queryFn: async () => (await idb.get<DraftPR>(draftPRKey({ projectId, branchName }))) ?? null,
	});

export const usePersistDraftPR = () =>
	useMutation({
		mutationFn: ({
			projectId,
			branchName,
			draft,
		}: {
			projectId: string;
			branchName: string;
			draft: DraftPR;
		}) => idb.set(draftPRKey({ projectId, branchName }), draft),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(
				draftPRQueryOptions({ projectId: input.projectId, branchName: input.branchName }).queryKey,
				input.draft,
			),
	});

export const useDeleteDraftPR = () =>
	useMutation({
		mutationFn: ({ projectId, branchName }: { projectId: string; branchName: string }) =>
			idb.del(draftPRKey({ projectId, branchName })),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(
				draftPRQueryOptions({ projectId: input.projectId, branchName: input.branchName }).queryKey,
				null,
			),
	});
