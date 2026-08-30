import { treeChangeDiffsQueryOptions } from "#ui/api/queries.ts";
import { weakFileParentIdentityKey, type FileParent } from "#ui/addresses.ts";
import { reviewedFilesQueryOptions, type ReviewedFileVersions } from "#ui/reviewed-files.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQueries, useQuery } from "@tanstack/react-query";
import { prepareDiffFiles, type PreparedDiffFile } from "./diff-view.ts";

/**
 * The paths whose diff, as it currently stands, has been reviewed.
 *
 * Reviewing records the version it saw, so a file edited since is not reviewed
 * any more — the state its diff header draws as indeterminate.
 */
export const reviewedPaths = (
	prepared: Array<PreparedDiffFile>,
	reviewedFiles: ReviewedFileVersions,
): ReadonlySet<string> =>
	new Set(
		prepared
			.filter(({ change, version }) => reviewedFiles.get(change.path)?.has(version))
			.map(({ change }) => change.path),
	);

/**
 * The same, for a list that holds changes but no diffs — the versions can only
 * be had from the patches.
 *
 * Only the files carrying a reviewed entry are fetched, which is what the
 * reader has worked through rather than everything in the tree, and those
 * queries are the ones the diff pane loads anyway, so they usually come from
 * the cache. Nothing reviewed means nothing fetched.
 */
export const useReviewedPaths = ({
	projectId,
	fileParent,
	changes,
}: {
	projectId: string;
	fileParent: FileParent;
	changes: Array<TreeChange>;
}): ReadonlySet<string> => {
	const { data: reviewedFiles } = useQuery(
		reviewedFilesQueryOptions(projectId, weakFileParentIdentityKey(fileParent)),
	);
	const reviewedChanges = changes.filter((change) => reviewedFiles?.has(change.path));

	return useQueries({
		queries: reviewedChanges.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
		// Folded down in `combine` rather than in render: react-query caches the
		// result against this closure, so the set keeps its identity while the
		// diffs do.
		combine: (results) =>
			reviewedFiles === undefined
				? EMPTY_REVIEWED_PATHS
				: reviewedPaths(
						prepareDiffFiles({
							fileParent,
							changes: reviewedChanges,
							treeChangeDiffs: results.map((result) => result.data ?? null),
						}),
						reviewedFiles,
					),
	});
};

const EMPTY_REVIEWED_PATHS: ReadonlySet<string> = new Set();
