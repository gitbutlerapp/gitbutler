import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { getAction, type ShortcutBinding } from "#ui/shortcuts.ts";
import { isTypingTarget } from "#ui/routes/project/$id/-shared.tsx";
import { AbsorptionTarget } from "@gitbutler/but-sdk";
import { useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";
import { Dispatch, useEffect, useEffectEvent } from "react";
import { type Editing } from "./-Editing.ts";
import {
	commitItem,
	type Item,
	CommitItem,
	getParentSection,
	SegmentItem,
	ChangesItem,
	BaseCommitItem,
} from "./-Item.ts";
import {
	getAdjacentPath,
	getAdjacentItem,
	getAdjacentSection,
	type NavigationModel,
} from "./-Selection.ts";
import { getFocus, type ProjectLayoutState } from "#ui/routes/project/$id/-state/layout.ts";
import { type ProjectStateAction } from "#ui/routes/project/$id/-state/project.ts";

type SelectionAction =
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "PreviousSection" }
	| { _tag: "NextSection" }
	| { _tag: "FocusPreview" }
	| { _tag: "ToggleFullscreenPreview" }
	| { _tag: "TogglePreview" };

type ChangesAction = SelectionAction | { _tag: "Absorb" };

type CommitSummaryAction = SelectionAction | { _tag: "EditMessage" } | { _tag: "OpenDetails" };

type CommitDetailsAction = SelectionAction | { _tag: "CloseDetails" };

type PreviewAction =
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "FocusPrimary" }
	| { _tag: "ToggleFullscreenPreview" }
	| { _tag: "CloseFullscreenPreview" }
	| { _tag: "TogglePreview" };

export const togglePreviewBinding: ShortcutBinding<SelectionAction> = {
	id: "toggle-preview",
	description: "Preview",
	keys: ["p"],
	action: { _tag: "TogglePreview" },
	repeat: false,
};

export const toggleFullscreenPreviewBinding: ShortcutBinding<SelectionAction> = {
	id: "toggle-fullscreen-preview",
	description: "Fullscreen preview",
	keys: ["d"],
	action: { _tag: "ToggleFullscreenPreview" },
	repeat: false,
};

const focusPreviewBinding: ShortcutBinding<SelectionAction> = {
	id: "focus-preview",
	description: "Focus preview",
	keys: ["Ctrl+l"],
	action: { _tag: "FocusPreview" },
	repeat: false,
};

const focusPrimaryBinding: ShortcutBinding<PreviewAction> = {
	id: "focus-primary",
	description: "Focus primary",
	keys: ["Ctrl+h"],
	action: { _tag: "FocusPrimary" },
	repeat: false,
};

const selectionBindings: Array<ShortcutBinding<SelectionAction>> = [
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
	{
		id: "previous-section",
		description: "Previous section",
		keys: ["Shift+ArrowUp", "Shift+k"],
		action: { _tag: "PreviousSection" },
		showInShortcutsBar: false,
	},
	{
		id: "next-section",
		description: "Next section",
		keys: ["Shift+ArrowDown", "Shift+j"],
		action: { _tag: "NextSection" },
		showInShortcutsBar: false,
	},
	focusPreviewBinding,
	toggleFullscreenPreviewBinding,
	togglePreviewBinding,
];

export const closeFullscreenPreviewBinding: ShortcutBinding<PreviewAction> = {
	id: "close-fullscreen-preview",
	description: "Close",
	keys: ["Escape"],
	action: { _tag: "CloseFullscreenPreview" },
	repeat: false,
};

const previewBindings: Array<ShortcutBinding<PreviewAction>> = [
	{
		id: "preview-move-up",
		description: "up",
		keys: ["ArrowUp", "k"],
		action: { _tag: "Move", offset: -1 },
	},
	{
		id: "preview-move-down",
		description: "down",
		keys: ["ArrowDown", "j"],
		action: { _tag: "Move", offset: 1 },
	},
	focusPrimaryBinding,
	{
		id: "preview-toggle-fullscreen",
		description: "Fullscreen preview",
		keys: ["d"],
		action: { _tag: "ToggleFullscreenPreview" },
		repeat: false,
	},
	{
		id: "preview-toggle",
		description: "Preview",
		keys: ["p"],
		action: { _tag: "TogglePreview" },
		repeat: false,
	},
];

const fullscreenPreviewBindings: Array<ShortcutBinding<PreviewAction>> = [
	...previewBindings
		// The preview panel is not visible as it sits behind the fullscreen dialog, so
		// there's no point having the toggle preview shortcut here.
		.filter((binding) => binding.action._tag !== "TogglePreview"),
	closeFullscreenPreviewBinding,
];

export const absorbChangesBinding: ShortcutBinding<ChangesAction> = {
	id: "changes-absorb",
	description: "Absorb",
	keys: ["a"],
	action: { _tag: "Absorb" },
	repeat: false,
};

const changesBindings: Array<ShortcutBinding<ChangesAction>> = [
	...selectionBindings,
	absorbChangesBinding,
];

const editCommitMessageBinding: ShortcutBinding<CommitSummaryAction> = {
	id: "commit-edit-message",
	description: "Reword",
	keys: ["Enter"],
	action: { _tag: "EditMessage" },
	repeat: false,
};

export const openCommitDetailsBinding: ShortcutBinding<CommitSummaryAction> = {
	id: "commit-open-details",
	description: "Open details",
	keys: ["ArrowRight", "l"],
	action: { _tag: "OpenDetails" },
	repeat: false,
};

const commitSummaryBindings: Array<ShortcutBinding<CommitSummaryAction>> = [
	...selectionBindings,
	editCommitMessageBinding,
	openCommitDetailsBinding,
];

export const closeCommitDetailsBinding: ShortcutBinding<CommitDetailsAction> = {
	id: "commit-close-details",
	description: "Close details",
	keys: ["ArrowLeft", "Escape"],
	action: { _tag: "CloseDetails" },
	repeat: false,
};

const commitDetailsBindings: Array<ShortcutBinding<CommitDetailsAction>> = [
	...selectionBindings,
	closeCommitDetailsBinding,
];

type BranchAction = SelectionAction | { _tag: "RenameBranch" };

const branchBindings: Array<ShortcutBinding<BranchAction>> = [
	...selectionBindings,
	{
		id: "segment-rename-branch",
		description: "Rename",
		keys: ["Enter"],
		action: { _tag: "RenameBranch" },
		repeat: false,
	},
];

type CommitEditingMessageAction = { _tag: "Save" } | { _tag: "Cancel" };

export const commitEditingMessageBindings: Array<ShortcutBinding<CommitEditingMessageAction>> = [
	{
		id: "commit-editing-message-save",
		description: "Save",
		keys: ["Enter"],
		action: { _tag: "Save" },
		repeat: false,
	},
	{
		id: "commit-editing-message-cancel",
		description: "Cancel",
		keys: ["Escape"],
		action: { _tag: "Cancel" },
		repeat: false,
	},
];

export const handleCommitEditingMessageKeyDown = ({
	event,
	onSave,
	onCancel,
}: {
	event: KeyboardEvent;
	onSave: () => void;
	onCancel: () => void;
}) => {
	const action = getAction(commitEditingMessageBindings, event);
	if (!action) return;

	event.preventDefault();

	Match.value(action).pipe(
		Match.tagsExhaustive({
			Save: onSave,
			Cancel: onCancel,
		}),
	);
};

type RenameBranchAction = { _tag: "Save" } | { _tag: "Cancel" };

export const renameBranchBindings: Array<ShortcutBinding<RenameBranchAction>> = [
	{
		id: "rename-branch-save",
		description: "Save",
		keys: ["Enter"],
		action: { _tag: "Save" },
		repeat: false,
	},
	{
		id: "rename-branch-cancel",
		description: "Cancel",
		keys: ["Escape"],
		action: { _tag: "Cancel" },
		repeat: false,
	},
];

export const handleRenameBranchKeyDown = ({
	event,
	onSave,
	onCancel,
}: {
	event: KeyboardEvent;
	onSave: () => void;
	onCancel: () => void;
}) => {
	const action = getAction(renameBranchBindings, event);
	if (!action) return;

	Match.value(action).pipe(
		Match.tagsExhaustive({
			Save: () => {
				event.preventDefault();
				onSave();
			},
			Cancel: () => {
				event.preventDefault();
				onCancel();
			},
		}),
	);
};

type Scope =
	| {
			_tag: "BaseCommit";
			bindings: Array<ShortcutBinding<SelectionAction>>;
			context: BaseCommitItem;
	  }
	| {
			_tag: "Changes";
			bindings: Array<ShortcutBinding<ChangesAction>>;
			context: ChangesItem;
	  }
	| {
			_tag: "CommitDetails";
			bindings: Array<ShortcutBinding<CommitDetailsAction>>;
			context: CommitItem;
	  }
	| {
			_tag: "CommitReword";
			bindings: Array<ShortcutBinding<CommitEditingMessageAction>>;
			context: CommitItem;
	  }
	| {
			_tag: "BranchRename";
			bindings: Array<ShortcutBinding<RenameBranchAction>>;
			context: SegmentItem;
	  }
	| {
			_tag: "CommitSummary";
			bindings: Array<ShortcutBinding<CommitSummaryAction>>;
			context: CommitItem;
	  }
	| {
			_tag: "Segment";
			bindings: Array<ShortcutBinding<SelectionAction>>;
			context: SegmentItem;
	  }
	| {
			_tag: "Branch";
			bindings: Array<ShortcutBinding<BranchAction>>;
			context: SegmentItem;
	  }
	| {
			_tag: "Preview";
			bindings: Array<ShortcutBinding<PreviewAction>>;
			context: { isFullscreen: boolean };
	  };

export const getScope = ({
	selectedItem,
	editing,
	layoutState,
}: {
	selectedItem: Item | null;
	editing: Editing | null;
	layoutState: ProjectLayoutState;
}): Scope | null => {
	if (getFocus(layoutState) === "preview")
		return {
			_tag: "Preview",
			bindings: layoutState.isFullscreenPreviewOpen ? fullscreenPreviewBindings : previewBindings,
			context: { isFullscreen: layoutState.isFullscreenPreviewOpen },
		};
	if (!selectedItem) return null;

	return Match.value(selectedItem).pipe(
		Match.tag(
			"Changes",
			(selectedItem): Scope => ({
				_tag: "Changes",
				bindings: changesBindings,
				context: selectedItem,
			}),
		),
		Match.tag("Commit", (selectedItem): Scope => {
			if (
				editing?._tag === "CommitMessage" &&
				editing.subject.stackId === selectedItem.stackId &&
				editing.subject.segmentIndex === selectedItem.segmentIndex &&
				editing.subject.commitId === selectedItem.commitId
			)
				return {
					_tag: "CommitReword",
					bindings: commitEditingMessageBindings,
					context: selectedItem,
				};

			return Match.value(selectedItem.mode).pipe(
				Match.tagsExhaustive({
					Details: (): Scope => ({
						_tag: "CommitDetails",
						bindings: commitDetailsBindings,
						context: selectedItem,
					}),
					Summary: (): Scope => ({
						_tag: "CommitSummary",
						bindings: commitSummaryBindings,
						context: selectedItem,
					}),
				}),
			);
		}),
		Match.tag(
			"BaseCommit",
			(selectedItem): Scope => ({
				_tag: "BaseCommit",
				bindings: selectionBindings,
				context: selectedItem,
			}),
		),
		Match.tag(
			"Segment",
			(selectedItem): Scope =>
				editing?._tag === "BranchName" &&
				editing.subject.stackId === selectedItem.stackId &&
				editing.subject.segmentIndex === selectedItem.segmentIndex
					? {
							_tag: "BranchRename",
							bindings: renameBranchBindings,
							context: selectedItem,
						}
					: selectedItem.branchName === null
						? {
								_tag: "Segment",
								bindings: selectionBindings,
								context: selectedItem,
							}
						: {
								_tag: "Branch",
								bindings: branchBindings,
								context: selectedItem,
							},
		),
		Match.exhaustive,
	);
};

export const getLabel = (scope: Scope): string =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => "Base commit",
			BranchRename: () => "Rename branch",
			Changes: () => "Changes",
			CommitDetails: () => "Commit details",
			CommitReword: () => "Reword commit",
			CommitSummary: () => "Commit",
			Branch: () => "Branch",
			Segment: () => "Segment",
			Preview: () => "Preview",
		}),
	);

export const useWorkspaceShortcuts = ({
	projectId,
	scope,
	selectItem,
	setEditing,
	navigationModel,
	requestAbsorptionPlan,
	dispatchProjectState,
	movePreviewSelection,
}: {
	projectId: string;
	scope: Scope | null;
	selectItem: (item: Item | null) => void;
	setEditing: (editing: Editing | null) => void;
	navigationModel: NavigationModel;
	requestAbsorptionPlan: (target: AbsorptionTarget) => void;
	dispatchProjectState: Dispatch<ProjectStateAction>;
	movePreviewSelection: (offset: -1 | 1) => void;
}) => {
	const queryClient = useQueryClient();

	const requestAbsorptionPlanForSelection = (selectedItem: ChangesItem) => {
		const worktreeChanges = queryClient.getQueryData(
			changesInWorktreeQueryOptions(projectId).queryKey,
		);
		if (!worktreeChanges) return;

		Match.value(selectedItem.mode).pipe(
			Match.tagsExhaustive({
				Details: ({ path }) => {
					const change = worktreeChanges.changes.find((change) => change.path === path);
					if (!change) return;
					requestAbsorptionPlan({
						type: "treeChanges",
						subject: {
							changes: [change],
							assigned_stack_id: selectedItem.stackId,
						},
					});
				},
				Summary: () => {
					const assignmentsByPath = new Set(
						worktreeChanges.assignments
							.filter((assignment) => assignment.stackId === selectedItem.stackId)
							.map((assignment) => assignment.path),
					);
					const changes = worktreeChanges.changes.filter((change) =>
						assignmentsByPath.has(change.path),
					);
					requestAbsorptionPlan({
						type: "treeChanges",
						subject: {
							changes,
							assigned_stack_id: selectedItem.stackId,
						},
					});
				},
			}),
		);
	};

	const moveCommitDetailsFile = (offset: -1 | 1, selectedItem: CommitItem) => {
		if (selectedItem.mode._tag !== "Details") return;

		const commitDetails = queryClient.getQueryData(
			commitDetailsWithLineStatsQueryOptions({
				projectId,
				commitId: selectedItem.commitId,
			}).queryKey,
		);
		if (!commitDetails) return;

		const paths = commitDetails.changes.map((change) => change.path);
		const currentPath = selectedItem.mode.path;
		const nextPath = getAdjacentPath({ paths, currentPath, offset });
		if (nextPath === null) return;

		selectItem(
			commitItem({
				...selectedItem,
				mode: { _tag: "Details", path: nextPath },
			}),
		);
	};

	const openCommitDetails = async (selectedItem: CommitItem) => {
		const commitDetails = await queryClient
			.fetchQuery(
				commitDetailsWithLineStatsQueryOptions({
					projectId,
					commitId: selectedItem.commitId,
				}),
			)
			.catch(() => null);
		if (!commitDetails) return;

		const firstPath = commitDetails.changes[0]?.path;

		selectItem(
			commitItem({
				...selectedItem,
				mode: firstPath === undefined ? { _tag: "Details" } : { _tag: "Details", path: firstPath },
			}),
		);
	};

	const move = (offset: -1 | 1, selectedItem: Item) =>
		selectItem(getAdjacentItem(navigationModel, selectedItem, offset));
	const previousSection = (selectedItem: Item) =>
		selectItem(
			getParentSection(selectedItem) ?? getAdjacentSection(navigationModel, selectedItem, -1),
		);
	const nextSection = (selectedItem: Item) =>
		selectItem(getAdjacentSection(navigationModel, selectedItem, 1));

	const handleSelectionAction = (action: SelectionAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Move: ({ offset }) => move(offset, selectedItem),
				PreviousSection: () => previousSection(selectedItem),
				NextSection: () => nextSection(selectedItem),
				FocusPreview: () => dispatchProjectState({ _tag: "FocusPreview" }),
				ToggleFullscreenPreview: () => dispatchProjectState({ _tag: "ToggleFullscreenPreview" }),
				TogglePreview: () => dispatchProjectState({ _tag: "TogglePreview" }),
			}),
		);

	const handlePreviewAction = (action: PreviewAction) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Move: ({ offset }) => movePreviewSelection(offset),
				FocusPrimary: () => dispatchProjectState({ _tag: "FocusPrimary" }),
				ToggleFullscreenPreview: () => dispatchProjectState({ _tag: "ToggleFullscreenPreview" }),
				CloseFullscreenPreview: () => dispatchProjectState({ _tag: "CloseFullscreenPreview" }),
				TogglePreview: () => dispatchProjectState({ _tag: "TogglePreview" }),
			}),
		);

	const handleChangesAction = (action: ChangesAction, selectedItem: ChangesItem) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => requestAbsorptionPlanForSelection(selectedItem),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Changes", ...selectedItem })),
		);

	const handleCommitSummaryAction = (action: CommitSummaryAction, selectedItem: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				EditMessage: () => setEditing({ _tag: "CommitMessage", subject: selectedItem }),
				OpenDetails: () => {
					void openCommitDetails(selectedItem);
				},
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Commit", ...selectedItem })),
		);

	const handleCommitDetailsAction = (action: CommitDetailsAction, selectedItem: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				Move: ({ offset }) => moveCommitDetailsFile(offset, selectedItem),
				CloseDetails: () => selectItem(commitItem({ ...selectedItem, mode: { _tag: "Summary" } })),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Commit", ...selectedItem })),
		);

	const handleBranchAction = (action: BranchAction, selectedItem: SegmentItem) =>
		Match.value(action).pipe(
			Match.tags({
				RenameBranch: () => {
					setEditing({
						_tag: "BranchName",
						subject: {
							stackId: selectedItem.stackId,
							segmentIndex: selectedItem.segmentIndex,
						},
					});
				},
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Segment", ...selectedItem })),
		);

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented) return;
		if (isTypingTarget(event.target)) return;

		if (!scope) return;

		Match.value(scope).pipe(
			Match.tagsExhaustive({
				Changes: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangesAction(action, scope.context);
				},
				BaseCommit: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleSelectionAction(action, { _tag: "BaseCommit", ...scope.context });
				},
				Segment: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleSelectionAction(action, { _tag: "Segment", ...scope.context });
				},
				Branch: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleBranchAction(action, scope.context);
				},
				Preview: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handlePreviewAction(action);
				},
				BranchRename: () => undefined,
				CommitSummary: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleCommitSummaryAction(action, scope.context);
				},
				CommitDetails: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleCommitDetailsAction(action, scope.context);
				},
				CommitReword: () => undefined,
			}),
		);
	});

	useEffect(() => {
		window.addEventListener("keydown", handleKeyDown);

		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, []);
};
