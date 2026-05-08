// TODO: how would we render a shortcuts palette even for non-rendered / non-selected?
// TODO: cut, move, etc
// TODO: shift+z, enter on changes section + escape
// TODO: enter on file

import { FC } from "react";
import {
	branchOperand,
	BranchOperand,
	changesSectionOperand,
	commitOperand,
	CommitOperand,
	FileOperand,
	StackOperand,
} from "#ui/operands.ts";
import {
	projectActions,
	selectProjectOutlineModeState,
	selectProjectPanelsState,
	selectProjectSelectionFiles,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { RegisterableHotkey, UseHotkeyDefinition, useHotkeys } from "@tanstack/react-hotkeys";
import { Match } from "effect";
import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { absorptionPlanQueryOptions, changesInWorktreeQueryOptions } from "#ui/api/queries.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { PickerDialog, PickerDialogGroup } from "#ui/ui/PickerDialog/PickerDialog.tsx";
import { Keys } from "#ui/ui/Keys.tsx";
import { isNonEmptyArray, reverse } from "effect/Array";
import { focusPanel, useFocusedProjectPanel } from "#ui/panels.ts";
import type { CommandGroup } from "#ui/commands/groups.ts";
import {
	commitDiscardMutationOptions,
	commitInsertBlankMutationOptions,
	unapplyStackMutationOptions,
} from "#ui/api/mutations.ts";
import { AbsorptionTarget } from "@gitbutler/but-sdk";
import { isPanelVisible } from "#ui/panels/state.ts";

type Item = {
	id: string;
	name: string;
	action: () => void;
	hotkeys: Array<RegisterableHotkey>;
};

type CommandPaletteGroup = PickerDialogGroup<Item> & {
	value: CommandGroup;
};

const useGlobalItems = (projectId: string): Array<CommandPaletteGroup> => {
	const dispatch = useAppDispatch();
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);
	const openApplyBranchPicker = () => {
		dispatch(projectActions.openApplyBranchPicker({ projectId }));
	};
	const toggleDetails = () => {
		if (focusedPanel === "details" && isPanelVisible(panelsState, "details")) {
			const detailsPanelIndex = panelsState.visiblePanels.indexOf("details");
			const nextPanel = panelsState.visiblePanels[detailsPanelIndex - 1];
			if (nextPanel !== undefined) focusPanel(nextPanel);
		}

		dispatch(projectActions.togglePanel({ projectId, panel: "details" }));
	};
	return [
		{
			value: "Global",
			items: [
				{
					id: "apply-branch",
					name: "Apply branch",
					action: openApplyBranchPicker,
					hotkeys: ["Shift+A"],
				},
				{
					id: "details",
					name: "Details",
					action: toggleDetails,
					hotkeys: ["D"],
				},
			],
		},
	];
};

const useOutlineItems = (projectId: string): Array<CommandPaletteGroup> => {
	const dispatch = useAppDispatch();
	const openBranchPicker = () => {
		dispatch(projectActions.openBranchPicker({ projectId }));
	};
	const selectChanges = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
	};
	const selectChangesAndFocusOutline = () => {
		selectChanges();
		focusPanel("outline");
	};
	return [
		{
			value: "Outline",
			items: [
				{
					id: "outline-select-branch",
					name: "Select branch",
					action: openBranchPicker,
					hotkeys: ["T"],
				},
				{
					id: "outline-select-changes",
					name: "Select changes",
					action: selectChangesAndFocusOutline,
					hotkeys: ["Z"],
				},
			],
		},
	];
};

const useChangesSectionItems = (projectId: string): Array<CommandPaletteGroup> => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const openAbsorptionDialog = (target: AbsorptionTarget) => {
		// Before opening the dialog, warm cache to avoid showing loading states in
		// the dialog itself. This also ensures we don't show a stale absorption
		// plan whilst the dialog revalidates.
		void queryClient.prefetchQuery(absorptionPlanQueryOptions({ projectId, target })).then(() => {
			dispatch(projectActions.openAbsorptionDialog({ projectId, target }));
		});
	};
	return [
		{
			value: "Changes",
			items:
				worktreeChanges.changes.length > 0
					? ([
							{
								id: "changes-section-absorb",
								name: "Absorb",
								action: () => {
									openAbsorptionDialog({ type: "all" });
								},
								hotkeys: ["A"],
							},
						] satisfies Array<Item>)
					: [],
		},
	];
};

const useStackItems = (projectId: string, stack: StackOperand): Array<CommandPaletteGroup> => {
	const unapplyStack = useMutation(unapplyStackMutationOptions);
	const unapply = () => {
		unapplyStack.mutate({ projectId, stackId: stack.stackId });
	};

	return [
		{
			value: "Stack",
			items: [
				{
					id: "stack-unapply",
					name: "Unapply",
					action: unapply,
					hotkeys: [],
				},
			],
		},
	];
};

const useBranchItems = (projectId: string, branch: BranchOperand): Array<CommandPaletteGroup> => {
	const dispatch = useAppDispatch();

	const rename = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: branchOperand(branch) }));
		dispatch(projectActions.startRenameBranch({ projectId, branch }));
		focusPanel("outline");
	};

	return [
		{
			value: "Branch",
			items: [
				{
					id: "branch-rename",
					name: "Rename",
					action: rename,
					hotkeys: ["Enter"],
				},
			],
		},
	];
};

const useCommitItems = (projectId: string, commit: CommitOperand): Array<CommandPaletteGroup> => {
	const dispatch = useAppDispatch();

	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const commitDiscard = useMutation(commitDiscardMutationOptions);

	const reword = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: commitOperand(commit) }));
		dispatch(projectActions.startRewordCommit({ projectId, commit }));
		focusPanel("outline");
	};

	const insertBlankCommitAbove = () => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.commitId },
			side: "above",
			dryRun: false,
		});
	};

	const insertBlankCommitBelow = () => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.commitId },
			side: "below",
			dryRun: false,
		});
	};

	const deleteCommit = () => {
		commitDiscard.mutate({
			projectId,
			subjectCommitId: commit.commitId,
			dryRun: false,
		});
	};

	return [
		{
			value: "Commit",
			items: [
				{
					id: "commit-reword",
					name: "Reword",
					action: reword,
					hotkeys: ["Enter"],
				},
				{
					id: "commit-insert-above",
					name: "Insert blank commit above",
					action: insertBlankCommitAbove,
					hotkeys: [],
				},
				{
					id: "commit-insert-below",
					name: "Insert blank commit below",
					action: insertBlankCommitBelow,
					hotkeys: [],
				},
				{
					id: "commit-delete",
					name: "Delete",
					action: deleteCommit,
					hotkeys: [],
				},
			],
		},
	];
};

const useChangesFileItems = (projectId: string, file: FileOperand): Array<CommandPaletteGroup> => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const openAbsorptionDialog = (target: AbsorptionTarget) => {
		// Before opening the dialog, warm cache to avoid showing loading states in
		// the dialog itself. This also ensures we don't show a stale absorption
		// plan whilst the dialog revalidates.
		void queryClient.prefetchQuery(absorptionPlanQueryOptions({ projectId, target })).then(() => {
			dispatch(projectActions.openAbsorptionDialog({ projectId, target }));
		});
	};

	const change = worktreeChanges.changes.find((candidate) => candidate.path === file.path);

	return [
		{
			value: "Changes file",
			items: change
				? [
						{
							id: "changes-file-absorb",
							name: "Absorb",
							action: () => {
								openAbsorptionDialog({
									type: "treeChanges",
									subject: {
										changes: [change],
										assignedStackId: null,
									},
								});
							},
							hotkeys: ["A"],
						},
					]
				: [],
		},
	];
};

const CommandPaletteFromItems: FC<{
	open: boolean;
	items?: Array<CommandPaletteGroup>;
	projectId: string;
}> = ({ open, items: itemsProp, projectId }) => {
	const dispatch = useAppDispatch();

	const items: Array<CommandPaletteGroup> = [
		...(itemsProp ?? []),
		...useOutlineItems(projectId),
		...useGlobalItems(projectId),
	];

	const runItem = (item: Item) => {
		dispatch(projectActions.closeDialog({ projectId }));
		item.action();
	};

	useHotkeys(
		reverse(items).flatMap((group) =>
			group.items.flatMap((item) =>
				item.hotkeys.map(
					(hotkey): UseHotkeyDefinition => ({
						hotkey,
						callback: () => runItem(item),
						options: {
							conflictBehavior: "replace",
							meta: {
								group: group.value,
								name: item.name,
								id: item.id,
							},
						},
					}),
				),
			),
		),
	);

	return (
		<PickerDialog
			ariaLabel="Command palette"
			closeLabel="Close command palette"
			emptyLabel="No commands found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.name}
			getItemType={(x) => isNonEmptyArray(x.hotkeys) && <Keys hotkey={x.hotkeys[0]} />}
			items={items}
			open={open}
			onOpenChange={(open) => {
				if (!open) dispatch(projectActions.closeDialog({ projectId }));
			}}
			onSelectItem={runItem}
			placeholder="Search commands…"
		/>
	);
};

const ChangesSection: FC<{ open: boolean; projectId: string }> = ({ open, projectId }) => (
	<CommandPaletteFromItems
		open={open}
		items={useChangesSectionItems(projectId)}
		projectId={projectId}
	/>
);

const Stack: FC<{ open: boolean; projectId: string; stack: StackOperand }> = ({
	open,
	projectId,
	stack,
}) => (
	<CommandPaletteFromItems
		open={open}
		items={useStackItems(projectId, stack)}
		projectId={projectId}
	/>
);

const Branch: FC<{ open: boolean; projectId: string; branch: BranchOperand }> = ({
	open,
	projectId,
	branch,
}) => (
	<CommandPaletteFromItems
		open={open}
		items={[
			...useBranchItems(projectId, branch),
			...useStackItems(projectId, { stackId: branch.stackId }),
		]}
		projectId={projectId}
	/>
);

const Commit: FC<{ open: boolean; projectId: string; commit: CommitOperand }> = ({
	open,
	projectId,
	commit,
}) => (
	<CommandPaletteFromItems
		open={open}
		items={[
			...useCommitItems(projectId, commit),
			// TODO: branch items
			...useStackItems(projectId, { stackId: commit.stackId }),
		]}
		projectId={projectId}
	/>
);

const ChangesFile: FC<{ open: boolean; projectId: string; file: FileOperand }> = ({
	open,
	projectId,
	file,
}) => (
	<CommandPaletteFromItems
		open={open}
		items={[...useChangesFileItems(projectId, file), ...useChangesSectionItems(projectId)]}
		projectId={projectId}
	/>
);

export const CommandPalette: FC<{ open: boolean; projectId: string }> = ({ open, projectId }) => {
	const outlineSelection = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);
	const filesSelection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);

	const focusedSelection = Match.value(focusedPanel).pipe(
		Match.when("outline", () => outlineSelection),
		Match.when("files", () => filesSelection),
		Match.orElse(() => null),
	);

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	if (outlineMode._tag !== "Default")
		return <CommandPaletteFromItems open={open} projectId={projectId} />;

	return Match.value(focusedSelection).pipe(
		Match.when({ _tag: "ChangesSection" }, () => (
			<ChangesSection open={open} projectId={projectId} />
		)),
		Match.when({ _tag: "Stack" }, (stack) => (
			<Stack open={open} projectId={projectId} stack={stack} />
		)),
		Match.when({ _tag: "Branch" }, (branch) => (
			<Branch open={open} projectId={projectId} branch={branch} />
		)),
		Match.when({ _tag: "Commit" }, (commit) => (
			<Commit open={open} projectId={projectId} commit={commit} />
		)),
		Match.when({ _tag: "File", parent: { _tag: "Changes" } }, (file) => (
			<ChangesFile open={open} projectId={projectId} file={file} />
		)),
		Match.when({ _tag: "File", parent: { _tag: "Branch" } }, (file) => (
			<Branch open={open} projectId={projectId} branch={file.parent} />
		)),
		Match.when({ _tag: "File", parent: { _tag: "Commit" } }, (file) => (
			<Commit open={open} projectId={projectId} commit={file.parent} />
		)),
		Match.orElse(() => <CommandPaletteFromItems open={open} projectId={projectId} />),
	);
};
