import { queryOptions, useMutation } from "@tanstack/react-query";
import * as idb from "idb-keyval";

/** Positively reviewed diff versions keyed by file path within one file-parent context. */
export type ReviewedFileVersions = Map<string, Set<number>>;

type ReviewedFile = { path: string; version: number };

const reviewedFilesKey = (projectId: string, contextId: string): string =>
	`reviewed_files:v1:${projectId}:${contextId}`;

export const reviewedFilesQueryOptions = (projectId: string, contextId: string) =>
	queryOptions({
		queryKey: ["reviewedFiles", projectId, contextId],
		queryFn: async (): Promise<ReviewedFileVersions> =>
			(await idb.get<ReviewedFileVersions>(reviewedFilesKey(projectId, contextId))) ?? new Map(),
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

export type SetFilesReviewedInput = {
	projectId: string;
	contextId: string;
	files: Array<ReviewedFile>;
	reviewed: boolean;
};

export const useSetFilesReviewed = () =>
	useMutation({
		mutationFn: async ({ projectId, contextId, files, reviewed }: SetFilesReviewedInput) =>
			idb.update<ReviewedFileVersions>(reviewedFilesKey(projectId, contextId), (reviewedFiles) =>
				updateReviewedFileVersions(reviewedFiles ?? new Map(), files, reviewed),
			),
		onMutate: (input, ctx) => {
			const queryKey = reviewedFilesQueryOptions(input.projectId, input.contextId).queryKey;
			const previous = ctx.client.getQueryData<ReviewedFileVersions>(queryKey);

			ctx.client.setQueryData(queryKey, (reviewedFiles: ReviewedFileVersions | undefined) =>
				updateReviewedFileVersions(reviewedFiles ?? new Map(), input.files, input.reviewed),
			);

			return previous;
		},
		onError: (_error, input, previous, ctx) => {
			ctx.client.setQueryData(
				reviewedFilesQueryOptions(input.projectId, input.contextId).queryKey,
				previous ?? new Map(),
			);
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
		mutationFn: async ({ projectId, contextId, paths }: PruneReviewedFilesInput) =>
			idb.update<ReviewedFileVersions>(reviewedFilesKey(projectId, contextId), (reviewedFiles) =>
				pruneReviewedFiles(reviewedFiles ?? new Map(), paths),
			),
		onMutate: (input, ctx) => {
			const queryKey = reviewedFilesQueryOptions(input.projectId, input.contextId).queryKey;
			const previous = ctx.client.getQueryData<ReviewedFileVersions>(queryKey);

			ctx.client.setQueryData(queryKey, (reviewedFiles: ReviewedFileVersions | undefined) =>
				pruneReviewedFiles(reviewedFiles ?? new Map(), input.paths),
			);

			return previous;
		},
		onError: (_error, input, previous, ctx) => {
			ctx.client.setQueryData(
				reviewedFilesQueryOptions(input.projectId, input.contextId).queryKey,
				previous ?? new Map(),
			);
		},
	});
