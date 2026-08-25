import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { addressEquals, type FileParent, type Address, addressFileParent } from "#ui/addresses.ts";
import { type QueryClient, useQueries, useQuery } from "@tanstack/react-query";
import type {
	CommitDetails,
	DiffSpec,
	HunkHeader,
	TreeChange,
	WorktreeChanges,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { diffSpecHunkHeadersForLineSelection } from "#ui/hunk.ts";

export const createDiffSpec = (change: TreeChange, hunkHeaders: Array<HunkHeader>): DiffSpec => ({
	pathBytes: change.pathBytes,
	previousPathBytes:
		change.status.type === "Rename" ? change.status.subject.previousPathBytes : null,
	hunkHeaders:
		change.status.type === "Addition" || change.status.type === "Deletion" ? [] : hunkHeaders,
});

/**
 * Which form a hunk's headers take. `commit` names only the selected lines, which is what keeping
 * them somewhere else — committing, moving, absorbing — asks for; `discard` names them within the
 * whole hunk, which is what taking them away — discarding, uncommitting — asks for. Left unsaid it
 * follows from the parent, the form moving a hunk wants: uncommitted lines are being committed,
 * committed ones taken back out of their commit.
 */
type HunkAction = "commit" | "discard";

const resolvedDiffSpecsFromAddress = ({
	address,
	worktreeChanges,
	commitDetails,
	hunkAction,
}: {
	address: Address;
	worktreeChanges: WorktreeChanges | undefined;
	commitDetails: CommitDetails | undefined;
	hunkAction: HunkAction | undefined;
}) =>
	Match.value(address).pipe(
		Match.withReturnType<Array<DiffSpec> | null>(),
		Match.tags({
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.withReturnType<Array<DiffSpec> | null>(),
					Match.tagsExhaustive({
						UncommittedChanges: () => {
							const change = worktreeChanges?.changes.find((candidate) => candidate.path === path);
							if (!change) return null;

							return [createDiffSpec(change, [])];
						},
						Commit: () => {
							const change = commitDetails?.changes.find((candidate) => candidate.path === path);
							if (!change) return null;

							return [createDiffSpec(change, [])];
						},
						Branch: () => null,
					}),
				),
			UncommittedChanges: () => {
				if (!worktreeChanges) return null;

				const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));
				return changes;
			},
			Hunk: (lineSelection) => {
				const { parent } = lineSelection;
				const changes = Match.value(parent.parent).pipe(
					Match.tagsExhaustive({
						UncommittedChanges: () => worktreeChanges?.changes,
						Commit: () => commitDetails?.changes,
						Branch: () => null,
					}),
				);
				if (!changes) return null;

				const change = changes.find((candidate) => candidate.path === parent.path);
				if (!change) return null;

				const hunkHeaders = diffSpecHunkHeadersForLineSelection(
					lineSelection,
					hunkAction ?? (parent.parent._tag === "UncommittedChanges" ? "commit" : "discard"),
				);

				return [createDiffSpec(change, hunkHeaders)];
			},
		}),
		Match.orElse(() => null),
	);

const commitIdFromParent = (parent: FileParent) =>
	Match.value(parent).pipe(
		Match.withReturnType<string | null>(),
		Match.tagsExhaustive({
			UncommittedChanges: () => null,
			Commit: ({ commitId }) => commitId,
			Branch: () => null,
		}),
	);

/**
 * Gets the file parent from an array of sibling sources, if any. Disparate file parents are not
 * currently supported.
 */
export const fileParentFromSources = (sources: Array<Address>): FileParent | null => {
	const [source, ...rest] = sources;
	if (!source) return null;

	const fileParent = addressFileParent(source);
	if (!fileParent) return null;

	if (
		rest.some((source) => {
			const otherFileParent = addressFileParent(source);
			return otherFileParent === null || !addressEquals(fileParent, otherFileParent);
		})
	)
		return null;

	return fileParent;
};

const resolvedDiffSpecsFromSources = ({
	sources,
	worktreeChanges,
	commitDetails,
	hunkAction,
}: {
	sources: Array<Address>;
	worktreeChanges: WorktreeChanges | undefined;
	commitDetails: CommitDetails | undefined;
	hunkAction: HunkAction | undefined;
}): Array<DiffSpec> | null => {
	const diffSpecsByPath = new Map<string, DiffSpec>();

	for (const address of sources) {
		const resolvedDiffSpecs = resolvedDiffSpecsFromAddress({
			address,
			worktreeChanges,
			commitDetails,
			hunkAction,
		});
		if (!resolvedDiffSpecs) return null;

		for (const diffSpec of resolvedDiffSpecs) {
			const path = diffSpec.pathBytes.join(",");
			const existing = diffSpecsByPath.get(path);

			if (!existing) diffSpecsByPath.set(path, diffSpec);
			else if (
				// One current path cannot originate from different rename sources.
				existing.previousPathBytes?.join(",") !== diffSpec.previousPathBytes?.join(",") ||
				// Empty headers select the whole file, which cannot mix with selected hunks.
				(existing.hunkHeaders.length === 0) !== (diffSpec.hunkHeaders.length === 0)
			)
				return null;
			else existing.hunkHeaders.push(...diffSpec.hunkHeaders);
		}
	}

	return diffSpecsByPath.values().toArray();
};

export const resolveDiffSpecs = async ({
	sources,
	projectId,
	queryClient,
	hunkAction,
}: {
	sources: Array<Address>;
	projectId: string;
	queryClient: QueryClient;
	hunkAction?: HunkAction;
}) => {
	const fileParent = fileParentFromSources(sources);
	if (!fileParent) return null;

	const commitId = commitIdFromParent(fileParent);
	const [worktreeChanges, commitDetails] = await Promise.all([
		queryClient.fetchQuery(changesInWorktreeQueryOptions(projectId)),
		commitId !== null
			? queryClient.fetchQuery(commitDetailsWithLineStatsQueryOptions({ projectId, commitId }))
			: undefined,
	]);

	return resolvedDiffSpecsFromSources({
		sources,
		worktreeChanges,
		commitDetails,
		hunkAction,
	});
};

export const useResolveDiffSpecs = ({
	sources,
	projectId,
	hunkAction,
}: {
	sources?: Array<Address>;
	projectId: string;
	hunkAction?: HunkAction;
}) => {
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const fileParent = fileParentFromSources(sources ?? []);
	const commitId = fileParent ? commitIdFromParent(fileParent) : null;
	const commitDetails = useQueries({
		queries: (commitId !== null ? [commitId] : []).map((commitId) =>
			commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
		),
		combine: ([result]) => result?.data,
	});

	if (!sources || !fileParent) return null;

	return resolvedDiffSpecsFromSources({
		sources,
		worktreeChanges,
		commitDetails,
		hunkAction,
	});
};
