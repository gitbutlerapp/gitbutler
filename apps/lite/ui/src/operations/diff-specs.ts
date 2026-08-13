import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { operandEquals, type FileParent, type Operand, operandFileParent } from "#ui/operands.ts";
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

const resolvedDiffSpecsFromOperand = ({
	operand,
	worktreeChanges,
	commitDetails,
}: {
	operand: Operand;
	worktreeChanges: WorktreeChanges | undefined;
	commitDetails: CommitDetails | undefined;
}) =>
	Match.value(operand).pipe(
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
					parent.parent._tag === "UncommittedChanges" ? "commit" : "discard",
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
const fileParentFromSources = (sources: Array<Operand>): FileParent | null => {
	const [source, ...rest] = sources;
	if (!source) return null;

	const fileParent = operandFileParent(source);
	if (!fileParent) return null;

	if (
		rest.some((source) => {
			const otherFileParent = operandFileParent(source);
			return otherFileParent === null || !operandEquals(fileParent, otherFileParent);
		})
	)
		return null;

	return fileParent;
};

const resolvedDiffSpecsFromSources = ({
	sources,
	worktreeChanges,
	commitDetails,
}: {
	sources: Array<Operand>;
	worktreeChanges: WorktreeChanges | undefined;
	commitDetails: CommitDetails | undefined;
}): Array<DiffSpec> | null => {
	const diffSpecsByPath = new Map<string, DiffSpec>();

	for (const operand of sources) {
		const resolvedDiffSpecs = resolvedDiffSpecsFromOperand({
			operand,
			worktreeChanges,
			commitDetails,
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
}: {
	sources: Array<Operand>;
	projectId: string;
	queryClient: QueryClient;
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
	});
};

export const useResolveDiffSpecs = ({
	sources,
	projectId,
}: {
	sources?: Array<Operand>;
	projectId: string;
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
	});
};
