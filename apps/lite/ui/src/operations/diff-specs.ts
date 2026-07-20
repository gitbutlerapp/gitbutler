import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { FileParent, Operand, operandFileParent } from "#ui/operands.ts";
import { type QueryClient, useQueries, useQuery } from "@tanstack/react-query";
import {
	CommitDetails,
	DiffSpec,
	HunkHeader,
	TreeChange,
	WorktreeChanges,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { diffSpecHunkHeadersForLineSelection } from "#ui/hunk.ts";
import type { HeadInfoIndex } from "#ui/api/ref-info.ts";

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

const changeIdFromParent = (parent: FileParent) =>
	Match.value(parent).pipe(
		Match.withReturnType<string | null>(),
		Match.tagsExhaustive({
			UncommittedChanges: () => null,
			Commit: ({ changeId }) => changeId,
			Branch: () => null,
		}),
	);

export const resolveDiffSpecs = async ({
	source,
	projectId,
	queryClient,
	headInfoIndex,
}: {
	source: Operand;
	projectId: string;
	queryClient: QueryClient;
	headInfoIndex: HeadInfoIndex;
}) => {
	const fileParent = operandFileParent(source);
	const changeId = fileParent ? changeIdFromParent(fileParent) : null;
	const commitCtx = changeId !== null ? headInfoIndex.commitContextById(changeId) : null;
	const [worktreeChanges, commitDetails] = await Promise.all([
		queryClient.fetchQuery(changesInWorktreeQueryOptions(projectId)),
		commitCtx != null
			? queryClient.fetchQuery(
					commitDetailsWithLineStatsQueryOptions({ projectId, commitId: commitCtx.commit.id }),
				)
			: Promise.resolve(undefined),
	]);

	return resolvedDiffSpecsFromOperand({
		operand: source,
		worktreeChanges,
		commitDetails,
	});
};

export const useResolveDiffSpecs = ({
	operand,
	projectId,
	headInfoIndex,
}: {
	operand?: Operand;
	projectId: string;
	headInfoIndex: HeadInfoIndex;
}) => {
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const fileParent = operand ? operandFileParent(operand) : null;
	const changeId = fileParent ? changeIdFromParent(fileParent) : null;
	const commitCtx = changeId !== null ? headInfoIndex.commitContextById(changeId) : null;
	const commitDetails = useQueries({
		queries: (commitCtx != null ? [commitCtx.commit.id] : []).map((commitId) =>
			commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
		),
		combine: ([result]) => result?.data,
	});

	if (!operand) return null;

	return resolvedDiffSpecsFromOperand({
		operand,
		worktreeChanges,
		commitDetails,
	});
};
