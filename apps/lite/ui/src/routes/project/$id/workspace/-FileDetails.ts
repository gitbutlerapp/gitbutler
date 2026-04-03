import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { assignedHunks } from "#ui/routes/project/$id/-shared.tsx";
import { type QueryClient } from "@tanstack/react-query";
import {
	changesDetailsItem,
	commitItem,
	detailsFileItem,
	detailsHunkItem,
	type ChangesItem,
	type CommitItem,
	type Item,
} from "./-Item.ts";

type Select = (selection: Item | null) => void;

export const openCommitFileDetails = async ({
	projectId,
	queryClient,
	select,
	selection,
}: {
	projectId: string;
	queryClient: QueryClient;
	select: Select;
	selection: CommitItem;
}) => {
	if (selection.mode._tag !== "Details" || selection.mode.item?._tag !== "File") return;

	const currentPath = selection.mode.item.path;
	select(
		commitItem({
			...selection,
			mode: { _tag: "Details", item: detailsFileItem(currentPath) },
		}),
	);

	const commitDetails = await queryClient
		.fetchQuery(
			commitDetailsWithLineStatsQueryOptions({
				projectId,
				commitId: selection.commitId,
			}),
		)
		.catch(() => null);
	if (!commitDetails) return;

	const change = commitDetails.changes.find((change) => change.path === currentPath);
	if (!change) return;

	const diff = await queryClient.fetchQuery(treeChangeDiffsQueryOptions({ projectId, change }));
	if (!diff || diff.type !== "Patch") return;

	const firstHunk = diff.subject.hunks[0];
	if (!firstHunk) return;

	select(
		commitItem({
			...selection,
			mode: { _tag: "Details", item: detailsHunkItem(currentPath, firstHunk) },
		}),
	);
};

export const closeCommitFileDetails = ({
	select,
	selection,
}: {
	select: Select;
	selection: CommitItem;
}) => {
	if (selection.mode._tag !== "Details" || selection.mode.item === null) return;

	select(
		commitItem({
			...selection,
			mode: { _tag: "Details", item: detailsFileItem(selection.mode.item.path) },
		}),
	);
};

export const openChangeFileDetails = async ({
	projectId,
	queryClient,
	select,
	selection,
}: {
	projectId: string;
	queryClient: QueryClient;
	select: Select;
	selection: ChangesItem;
}) => {
	if (selection.mode._tag !== "Details" || selection.mode.item._tag !== "File") return;

	const currentPath = selection.mode.item.path;
	select(changesDetailsItem(selection.stackId, detailsFileItem(currentPath)));

	const worktreeChanges = await queryClient
		.fetchQuery(changesInWorktreeQueryOptions(projectId))
		.catch(() => null);
	if (!worktreeChanges) return;

	const change = worktreeChanges.changes.find((change) => change.path === currentPath);
	if (!change) return;

	const diff = await queryClient.fetchQuery(treeChangeDiffsQueryOptions({ projectId, change }));
	if (!diff || diff.type !== "Patch") return;

	const assignments = worktreeChanges.assignments.filter(
		(assignment) =>
			(assignment.stackId ?? null) === selection.stackId && assignment.path === change.path,
	);
	const firstHunk = assignedHunks(diff.subject.hunks, assignments)[0];
	if (!firstHunk) return;

	select(changesDetailsItem(selection.stackId, detailsHunkItem(currentPath, firstHunk)));
};

export const closeChangeFileDetails = ({
	select,
	selection,
}: {
	select: Select;
	selection: ChangesItem;
}) => {
	if (selection.mode._tag !== "Details") return;

	select(changesDetailsItem(selection.stackId, detailsFileItem(selection.mode.item.path)));
};
