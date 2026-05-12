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

const createDiffSpec = (change: TreeChange, hunkHeaders: Array<HunkHeader>): DiffSpec => ({
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
		Match.withReturnType<Array<DiffSpec> | undefined>(),
		Match.tags({
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.withReturnType<Array<DiffSpec> | undefined>(),
					Match.tagsExhaustive({
						Changes: () => {
							const change = worktreeChanges?.changes.find((candidate) => candidate.path === path);
							if (!change) return undefined;

							return [createDiffSpec(change, [])];
						},
						Commit: () => {
							const change = commitDetails?.changes.find((candidate) => candidate.path === path);
							if (!change) return undefined;

							return [createDiffSpec(change, [])];
						},
						Branch: () => undefined,
					}),
				),
			ChangesSection: () => {
				if (!worktreeChanges) return undefined;

				const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));
				return changes;
			},
			Hunk: ({ parent, hunkHeader }) => {
				const changes = Match.value(parent.parent).pipe(
					Match.tagsExhaustive({
						Changes: () => worktreeChanges?.changes,
						Commit: () => commitDetails?.changes,
						Branch: () => undefined,
					}),
				);
				if (!changes) return undefined;

				const change = changes.find((candidate) => candidate.path === parent.path);
				if (!change) return undefined;

				return [createDiffSpec(change, [hunkHeader])];
			},
		}),
		Match.orElse(() => undefined),
	);

const commitIdFromParent = (parent: FileParent) =>
	Match.value(parent).pipe(
		Match.withReturnType<string | undefined>(),
		Match.tagsExhaustive({
			Changes: () => undefined,
			Commit: ({ commitId }) => commitId,
			Branch: () => undefined,
		}),
	);

export const resolveDiffSpecs = async ({
	source,
	projectId,
	queryClient,
}: {
	source?: Operand;
	projectId: string;
	queryClient: QueryClient;
}) => {
	if (!source) return undefined;

	const fileParent = operandFileParent(source);
	const commitId = fileParent ? commitIdFromParent(fileParent) : undefined;
	const [worktreeChanges, commitDetails] = await Promise.all([
		queryClient.fetchQuery(changesInWorktreeQueryOptions(projectId)),
		commitId !== undefined
			? queryClient.fetchQuery(commitDetailsWithLineStatsQueryOptions({ projectId, commitId }))
			: Promise.resolve(undefined),
	]);

	return resolvedDiffSpecsFromOperand({
		operand: source,
		worktreeChanges,
		commitDetails,
	});
};

export const useResolveDiffSpecs = ({
	source,
	projectId,
}: {
	source?: Operand;
	projectId: string;
}) => {
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const fileParent = source ? operandFileParent(source) : undefined;
	const commitId = fileParent ? commitIdFromParent(fileParent) : undefined;
	const conditionalQueries = useQueries({
		queries: (commitId !== undefined ? [commitId] : []).map((commitId) =>
			commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
		),
	});
	const commitDetails = conditionalQueries[0]?.data;

	if (!source) return undefined;

	return resolvedDiffSpecsFromOperand({
		operand: source,
		worktreeChanges,
		commitDetails,
	});
};
