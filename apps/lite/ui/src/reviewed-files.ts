import { queryOptions, useMutation } from "@tanstack/react-query";
import * as idb from "idb-keyval";

/** Positively reviewed diff versions keyed by file path within one file-parent context. */
export type ReviewedFileVersions = Map<string, Set<number>>;

type ReviewedFile = { path: string; version: number };

type ReviewScope =
	| { projectId: string; contextId: string }
	| { login: string; checkoutId: string; target: string };

const reviewedFilesKey = (scope: ReviewScope): string =>
	"projectId" in scope
		? `reviewed_files:v1:${scope.projectId}:${scope.contextId}`
		: `butDev:reviewed_files:v1:${JSON.stringify([scope.login, scope.checkoutId, scope.target])}`;

export const reviewedFilesQueryOptions = (scope: ReviewScope) =>
	queryOptions({
		// oxlint-disable-next-line @tanstack/query/exhaustive-deps -- Both scope variants are fully represented in their keys.
		queryKey:
			"projectId" in scope
				? [scope.projectId, "reviewedFiles", scope.contextId]
				: ["butDev", "reviewedFiles", scope.login, scope.checkoutId, scope.target],
		queryFn: async (): Promise<ReviewedFileVersions> =>
			(await idb.get<ReviewedFileVersions>(reviewedFilesKey(scope))) ?? new Map(),
	});

const updateReviewedFileVersions = (
	reviewedFiles: ReviewedFileVersions,
	files: Array<ReviewedFile>,
	reviewed: boolean,
): ReviewedFileVersions => {
	const next = new Map(reviewedFiles);
	for (const { path, version } of files) {
		const versions = next.get(path);
		if (reviewed) {
			if (!versions?.has(version)) next.set(path, new Set(versions).add(version));
			continue;
		}

		if (!versions?.has(version)) continue;
		const nextVersions = new Set(versions);
		nextVersions.delete(version);
		if (nextVersions.size === 0) next.delete(path);
		else next.set(path, nextVersions);
	}

	return next;
};

export type SetFilesReviewedInput = ReviewScope & {
	files: Array<ReviewedFile>;
	reviewed: boolean;
};

export const useSetFilesReviewed = () =>
	useMutation({
		mutationFn: async (input: SetFilesReviewedInput) =>
			idb.update<ReviewedFileVersions>(reviewedFilesKey(input), (reviewedFiles) =>
				updateReviewedFileVersions(reviewedFiles ?? new Map(), input.files, input.reviewed),
			),
		onMutate: (input, ctx) => {
			const queryKey = reviewedFilesQueryOptions(input).queryKey;
			const previous = ctx.client.getQueryData<ReviewedFileVersions>(queryKey);

			ctx.client.setQueryData(queryKey, (reviewedFiles: ReviewedFileVersions | undefined) =>
				updateReviewedFileVersions(reviewedFiles ?? new Map(), input.files, input.reviewed),
			);

			return previous;
		},
		onError: (_error, input, previous, ctx) => {
			ctx.client.setQueryData(reviewedFilesQueryOptions(input).queryKey, previous ?? new Map());
		},
	});

const pruneReviewedFiles = (
	reviewedFiles: ReviewedFileVersions,
	paths: Iterable<string>,
): ReviewedFileVersions => {
	const next = new Map(reviewedFiles);
	for (const path of paths) next.delete(path);
	return next;
};

type PruneReviewedFilesInput = {
	projectId: string;
	contextId: string;
	paths: Iterable<string>;
};

export const usePruneReviewedFiles = () =>
	useMutation({
		mutationFn: async (input: PruneReviewedFilesInput) =>
			idb.update<ReviewedFileVersions>(reviewedFilesKey(input), (reviewedFiles) =>
				pruneReviewedFiles(reviewedFiles ?? new Map(), input.paths),
			),
		onMutate: (input, ctx) => {
			const queryKey = reviewedFilesQueryOptions(input).queryKey;
			const previous = ctx.client.getQueryData<ReviewedFileVersions>(queryKey);

			ctx.client.setQueryData(queryKey, (reviewedFiles: ReviewedFileVersions | undefined) =>
				pruneReviewedFiles(reviewedFiles ?? new Map(), input.paths),
			);

			return previous;
		},
		onError: (_error, input, previous, ctx) => {
			ctx.client.setQueryData(reviewedFilesQueryOptions(input).queryKey, previous ?? new Map());
		},
	});
