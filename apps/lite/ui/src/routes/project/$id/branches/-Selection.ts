import { type ShortcutBinding } from "#ui/shortcuts.ts";
import { BranchIdentity, BranchListing } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { ShortcutsBarMode } from "../-ShortcutsBar";
import {
	branchSummaryItem,
	type CommitItem,
	commitSummaryItem,
	getParentItem,
	type Item,
	type BranchItem,
} from "./-Item.ts";

export type Selection = Item;
export type CommitSelection = CommitItem;

export const normalizeBranchSelection = (
	selection: Selection,
	branches: Array<BranchListing>,
): Selection | null => {
	const branch = branches.find((branch) => branch.name === selection.branchName);
	if (!branch) return null;

	return selection;
};

export const getDefaultSelection = (branches: Array<BranchListing>): Selection | null => {
	const firstBranch = branches[0];
	return firstBranch ? branchSummaryItem(firstBranch.name) : null;
};

export const getParentSelection = getParentItem;

export const getAdjacentBranchSelection = (
	branches: Array<BranchListing>,
	selection: Selection,
	offset: -1 | 1,
): Selection | null => {
	const currentIndex = branches.findIndex((branch) => branch.name === selection.branchName);
	if (currentIndex === -1 || branches.length === 0) return null;
	const nextBranch = branches[(currentIndex + offset + branches.length) % branches.length];
	return nextBranch ? branchSummaryItem(nextBranch.name) : null;
};

export const getAdjacentCommitSelection = ({
	branchName,
	commitIds,
	selection,
	offset,
}: {
	branchName: BranchIdentity;
	commitIds: Array<string>;
	selection: Selection;
	offset: -1 | 1;
}): Selection | null => {
	const selectedCommitId = getSelectedBranchCommitId({ commitIds, selection });
	if (selectedCommitId === undefined) return null;
	const currentIndex = commitIds.indexOf(selectedCommitId);
	if (currentIndex === -1) return null;
	const nextCommitId = commitIds[currentIndex + offset];
	return nextCommitId !== undefined ? commitSummaryItem(branchName, nextCommitId) : null;
};

export const getSelectedBranchCommitId = ({
	commitIds,
	selection,
}: {
	commitIds: Array<string>;
	selection: Selection;
}): string | undefined => {
	if (selection._tag === "Commit" && commitIds.includes(selection.commitId))
		return selection.commitId;
	return commitIds[0];
};

type MoveSelectionAction = { _tag: "Move"; offset: -1 | 1 };

export type SharedSelectionAction = { _tag: "NextBranch" } | { _tag: "PreviousBranch" };

type BranchSummarySelectionAction =
	| SharedSelectionAction
	| MoveSelectionAction
	| { _tag: "ExpandBranch" };
type BranchDetailsSelectionAction =
	| SharedSelectionAction
	| MoveSelectionAction
	| { _tag: "CloseBranch" };
type CommitSummarySelectionAction =
	| SharedSelectionAction
	| MoveSelectionAction
	| { _tag: "ExpandCommit" }
	| { _tag: "CloseBranch" };
type CommitDetailsSelectionAction =
	| SharedSelectionAction
	| MoveSelectionAction
	| { _tag: "CloseCommitDetails" };

const createSharedSelectionBindings = <Context>(): Array<
	ShortcutBinding<SharedSelectionAction, Context>
> => [
	{
		id: "next-branch",
		description: "next branch",
		keys: ["J"],
		action: { _tag: "NextBranch" },
	},
	{
		id: "previous-branch",
		description: "previous branch",
		keys: ["K"],
		action: { _tag: "PreviousBranch" },
	},
];

const createMoveSelectionBindings = <Context>(): Array<
	ShortcutBinding<MoveSelectionAction, Context>
> => [
	{
		id: "move-up",
		description: "up",
		keys: ["ArrowUp", "k"],
		action: { _tag: "Move", offset: -1 },
	},
	{
		id: "move-down",
		description: "down",
		keys: ["ArrowDown", "j"],
		action: { _tag: "Move", offset: 1 },
	},
];

export const branchSummarySelectionBindings: Array<
	ShortcutBinding<BranchSummarySelectionAction, BranchItem>
> = [
	...createMoveSelectionBindings<BranchItem>(),
	...createSharedSelectionBindings<BranchItem>(),
	{
		id: "branch-expand",
		description: "expand branch",
		keys: ["ArrowRight", "l"],
		action: { _tag: "ExpandBranch" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Summary",
	},
];

export const branchDetailsSelectionBindings: Array<
	ShortcutBinding<BranchDetailsSelectionAction, BranchItem>
> = [
	...createMoveSelectionBindings<BranchItem>(),
	...createSharedSelectionBindings<BranchItem>(),
	{
		id: "branch-close",
		description: "close branch",
		keys: ["ArrowLeft", "Escape"],
		action: { _tag: "CloseBranch" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Details",
	},
];

export const commitSummarySelectionBindings: Array<
	ShortcutBinding<CommitSummarySelectionAction, CommitItem>
> = [
	...createMoveSelectionBindings<CommitItem>(),
	...createSharedSelectionBindings<CommitItem>(),
	{
		id: "commit-expand",
		description: "details",
		keys: ["ArrowRight", "l"],
		action: { _tag: "ExpandCommit" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Summary",
	},
	{
		id: "commit-close-branch",
		description: "close branch",
		keys: ["ArrowLeft", "Escape"],
		action: { _tag: "CloseBranch" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Summary",
	},
];

export const commitDetailsSelectionBindings: Array<
	ShortcutBinding<CommitDetailsSelectionAction, CommitItem>
> = [
	...createMoveSelectionBindings<CommitItem>(),
	...createSharedSelectionBindings<CommitItem>(),
	{
		id: "commit-close-details",
		description: "close details",
		keys: ["ArrowLeft", "Escape"],
		action: { _tag: "CloseCommitDetails" },
		repeat: false,
		when: (selection) => selection.mode._tag === "Details",
	},
];

export const getShortcutsBarMode = ({
	selection,
}: {
	selection: Selection | null;
}): ShortcutsBarMode | null => {
	if (selection === null) return null;

	return Match.value(selection).pipe(
		Match.tag("Branch", (selection) =>
			Match.value(selection.mode).pipe(
				Match.tag(
					"Summary",
					(): ShortcutsBarMode => ({
						label: "branch",
						items: branchSummarySelectionBindings.filter(
							(binding) => binding.when?.(selection) ?? true,
						),
					}),
				),
				Match.tag(
					"Details",
					(): ShortcutsBarMode => ({
						label: "branch details",
						items: branchDetailsSelectionBindings.filter(
							(binding) => binding.when?.(selection) ?? true,
						),
					}),
				),
				Match.exhaustive,
			),
		),
		Match.tag("Commit", (selection) =>
			Match.value(selection.mode).pipe(
				Match.tag(
					"Summary",
					(): ShortcutsBarMode => ({
						label: "commit",
						items: commitSummarySelectionBindings.filter(
							(binding) => binding.when?.(selection) ?? true,
						),
					}),
				),
				Match.tag(
					"Details",
					(): ShortcutsBarMode => ({
						label: "commit details",
						items: commitDetailsSelectionBindings.filter(
							(binding) => binding.when?.(selection) ?? true,
						),
					}),
				),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);
};
