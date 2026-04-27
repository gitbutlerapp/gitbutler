import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { Item } from "#ui/routes/project/$id/workspace/Item.ts";
import { QueryClient } from "@tanstack/react-query";
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

const resolvedDiffSpecsFromItem = ({
	item,
	worktreeChanges,
	getCommitDetails,
}: {
	item: Item;
	worktreeChanges: WorktreeChanges | undefined;
	getCommitDetails: (commitId: string) => CommitDetails | undefined;
}) =>
	Match.value(item).pipe(
		Match.withReturnType<Array<DiffSpec> | null>(),
		Match.tags({
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.withReturnType<Array<DiffSpec> | null>(),
					Match.tagsExhaustive({
						Changes: () => {
							const change = worktreeChanges?.changes.find((candidate) => candidate.path === path);
							if (!change) return null;

							return [createDiffSpec(change, [])];
						},
						Commit: ({ commitId }) => {
							const change = getCommitDetails(commitId)?.changes.find(
								(candidate) => candidate.path === path,
							);
							if (!change) return null;

							return [createDiffSpec(change, [])];
						},
						Branch: () => null,
					}),
				),
			ChangesSection: () => {
				if (!worktreeChanges) return null;

				const changes = worktreeChanges.changes.map((change) => createDiffSpec(change, []));
				return changes;
			},
			Hunk: ({ parent, path, hunkHeader }) => {
				const changes = Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: () => worktreeChanges?.changes,
						Commit: ({ commitId }) => getCommitDetails(commitId)?.changes,
						Branch: () => null,
					}),
				);
				if (!changes) return null;

				const change = changes.find((candidate) => candidate.path === path);
				if (!change) return null;

				return [createDiffSpec(change, [hunkHeader])];
			},
		}),
		Match.orElse(() => null),
	);

export const resolveDiffSpecs = ({
	source,
	queryClient,
	projectId,
}: {
	source: Item;
	queryClient: QueryClient;
	projectId: string;
}) =>
	resolvedDiffSpecsFromItem({
		item: source,
		worktreeChanges: queryClient.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey),
		getCommitDetails: (commitId) =>
			queryClient.getQueryData(
				commitDetailsWithLineStatsQueryOptions({ projectId, commitId }).queryKey,
			),
	});
