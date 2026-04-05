import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { getAction, type ShortcutBinding } from "#ui/shortcuts.ts";
import { isTypingTarget } from "#ui/routes/project/$id/-shared.tsx";
import { AbsorptionTarget } from "@gitbutler/but-sdk";
import { useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";
import { Dispatch, RefObject, useEffect, useEffectEvent } from "react";
import {
	commitItem,
	type Item,
	type ChangeItem,
	CommitItem,
	getParentSection,
	SegmentItem,
	type ChangesSectionItem,
	BaseCommitItem,
} from "./-Item.ts";
import {
	getAdjacentPath,
	getAdjacentItem,
	getAdjacentSection,
	normalizeSelectedFile,
	type NavigationModel,
} from "./-Selection.ts";
import { getFocus, type ProjectLayoutState } from "#ui/routes/project/$id/-state/layout.ts";
import { type ProjectStateAction } from "#ui/routes/project/$id/-state/project.ts";
import { PreviewImperativeHandle } from "./route.tsx";

type ItemSelectionAction =
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "PreviousSection" }
	| { _tag: "NextSection" };

type PrimaryPanelAction =
	| ItemSelectionAction
	| { _tag: "FocusPreview" }
	| { _tag: "ToggleFullscreenPreview" }
	| { _tag: "TogglePreview" };

type ChangesAction = PrimaryPanelAction | { _tag: "Absorb" };

type CommitDefaultAction = PrimaryPanelAction | { _tag: "EditMessage" } | { _tag: "OpenDetails" };

type CommitDetailsAction = PrimaryPanelAction | { _tag: "CloseDetails" };

type HunkSelectionAction = { _tag: "Move"; offset: -1 | 1 };

type PreviewAction =
	| HunkSelectionAction
	| { _tag: "FocusPrimary" }
	| { _tag: "ToggleFullscreenPreview" }
	| { _tag: "ClosePreview" }
	| { _tag: "TogglePreview" };

export const togglePreviewBinding: ShortcutBinding<PrimaryPanelAction> = {
	id: "toggle-preview",
	description: "Preview",
	keys: ["p"],
	action: { _tag: "TogglePreview" },
	repeat: false,
};

export const toggleFullscreenPreviewBinding: ShortcutBinding<PrimaryPanelAction> = {
	id: "toggle-fullscreen-preview",
	description: "Fullscreen preview",
	keys: ["d"],
	action: { _tag: "ToggleFullscreenPreview" },
	repeat: false,
};

const focusPreviewBinding: ShortcutBinding<PrimaryPanelAction> = {
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

const primaryPanelBindings: Array<ShortcutBinding<PrimaryPanelAction>> = [
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

export const closePreviewBinding: ShortcutBinding<PreviewAction> = {
	id: "close-preview",
	description: "Close",
	keys: ["Escape"],
	action: { _tag: "ClosePreview" },
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
	closePreviewBinding,
];

const fullscreenPreviewBindings: Array<ShortcutBinding<PreviewAction>> = previewBindings
	// The preview panel is not visible as it sits behind the fullscreen dialog, so
	// there's no point having the toggle preview shortcut here.
	.filter((binding) => binding.action._tag !== "TogglePreview");

export const absorbChangesBinding: ShortcutBinding<ChangesAction> = {
	id: "changes-absorb",
	description: "Absorb",
	keys: ["a"],
	action: { _tag: "Absorb" },
	repeat: false,
};

const changesBindings: Array<ShortcutBinding<ChangesAction>> = [
	...primaryPanelBindings,
	absorbChangesBinding,
];

const editCommitMessageBinding: ShortcutBinding<CommitDefaultAction> = {
	id: "commit-edit-message",
	description: "Reword",
	keys: ["Enter"],
	action: { _tag: "EditMessage" },
	repeat: false,
};

export const openCommitDetailsBinding: ShortcutBinding<CommitDefaultAction> = {
	id: "commit-open-details",
	description: "Open details",
	keys: ["ArrowRight", "l"],
	action: { _tag: "OpenDetails" },
	repeat: false,
};

const commitDefaultBindings: Array<ShortcutBinding<CommitDefaultAction>> = [
	...primaryPanelBindings,
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
	...primaryPanelBindings,
	closeCommitDetailsBinding,
];

type BranchAction = PrimaryPanelAction | { _tag: "RenameBranch" };

const branchBindings: Array<ShortcutBinding<BranchAction>> = [
	...primaryPanelBindings,
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
			bindings: Array<ShortcutBinding<PrimaryPanelAction>>;
			context: BaseCommitItem;
	  }
	| {
			_tag: "Changes";
			bindings: Array<ShortcutBinding<ChangesAction>>;
			context: ChangesSectionItem;
	  }
	| {
			_tag: "Change";
			bindings: Array<ShortcutBinding<ChangesAction>>;
			context: ChangeItem;
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
			_tag: "CommitDefault";
			bindings: Array<ShortcutBinding<CommitDefaultAction>>;
			context: CommitItem;
	  }
	| {
			_tag: "Segment";
			bindings: Array<ShortcutBinding<PrimaryPanelAction>>;
			context: SegmentItem;
	  }
	| {
			_tag: "BranchDefault";
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
	layoutState,
}: {
	selectedItem: Item | null;
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
		Match.tag(
			"Change",
			(selectedItem): Scope => ({
				_tag: "Change",
				bindings: changesBindings,
				context: selectedItem,
			}),
		),
		Match.tag(
			"Commit",
			(selectedItem): Scope =>
				Match.value(selectedItem.mode).pipe(
					Match.tagsExhaustive({
						Reword: (): Scope => ({
							_tag: "CommitReword",
							bindings: commitEditingMessageBindings,
							context: selectedItem,
						}),
						Details: (): Scope => ({
							_tag: "CommitDetails",
							bindings: commitDetailsBindings,
							context: selectedItem,
						}),
						Default: (): Scope => ({
							_tag: "CommitDefault",
							bindings: commitDefaultBindings,
							context: selectedItem,
						}),
					}),
				),
		),
		Match.tag(
			"BaseCommit",
			(selectedItem): Scope => ({
				_tag: "BaseCommit",
				bindings: primaryPanelBindings,
				context: selectedItem,
			}),
		),
		Match.tag(
			"Segment",
			(selectedItem): Scope =>
				selectedItem.mode._tag === "Rename"
					? {
							_tag: "BranchRename",
							bindings: renameBranchBindings,
							context: selectedItem,
						}
					: selectedItem.branchName === null
						? {
								_tag: "Segment",
								bindings: primaryPanelBindings,
								context: selectedItem,
							}
						: {
								_tag: "BranchDefault",
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
			Change: () => "Change",
			Changes: () => "Changes",
			CommitDetails: () => "Commit details",
			CommitReword: () => "Reword commit",
			CommitDefault: () => "Commit",
			BranchDefault: () => "Branch",
			Segment: () => "Segment",
			Preview: () => "Preview",
		}),
	);

export const useWorkspaceShortcuts = ({
	projectId,
	scope,
	selectedFile,
	navigationModel,
	requestAbsorptionPlan,
	dispatchProjectState,
	previewRef,
}: {
	projectId: string;
	scope: Scope | null;
	selectedFile: string | null;
	navigationModel: NavigationModel;
	requestAbsorptionPlan: (target: AbsorptionTarget) => void;
	dispatchProjectState: Dispatch<ProjectStateAction>;
	previewRef: RefObject<PreviewImperativeHandle | null>;
}) => {
	const queryClient = useQueryClient();

	const requestAbsorptionPlanForSelection = (
		selectedItem: ({ _tag: "Changes" } & ChangesSectionItem) | ({ _tag: "Change" } & ChangeItem),
	) => {
		const worktreeChanges = queryClient.getQueryData(
			changesInWorktreeQueryOptions(projectId).queryKey,
		);
		if (!worktreeChanges) return;

		Match.value(selectedItem).pipe(
			Match.tagsExhaustive({
				Change: ({ path, stackId }) => {
					const change = worktreeChanges.changes.find((candidate) => candidate.path === path);
					if (!change) return;
					requestAbsorptionPlan({
						type: "treeChanges",
						subject: {
							changes: [change],
							assigned_stack_id: stackId,
						},
					});
				},
				Changes: ({ stackId }) => {
					const assignmentsByPath = new Set(
						worktreeChanges.assignments
							.filter((assignment) => assignment.stackId === stackId)
							.map((assignment) => assignment.path),
					);
					const changes = worktreeChanges.changes.filter((change) =>
						assignmentsByPath.has(change.path),
					);
					requestAbsorptionPlan({
						type: "treeChanges",
						subject: {
							changes,
							assigned_stack_id: stackId,
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
		const currentPath = normalizeSelectedFile({ paths: paths, selectedFile });
		const nextPath = getAdjacentPath({ paths, currentPath, offset });
		if (nextPath === null) return;

		dispatchProjectState({
			_tag: "SelectFile",
			file: nextPath,
		});
	};

	const openCommitDetails = (selectedItem: CommitItem) => {
		dispatchProjectState({
			_tag: "SelectItem",
			item: commitItem({
				...selectedItem,
				mode: { _tag: "Details" },
			}),
		});
	};

	const move = (offset: -1 | 1, selectedItem: Item) =>
		dispatchProjectState({
			_tag: "SelectItem",
			item: getAdjacentItem(navigationModel, selectedItem, offset),
		});
	const previousSection = (selectedItem: Item) =>
		dispatchProjectState({
			_tag: "SelectItem",
			item: getParentSection(selectedItem) ?? getAdjacentSection(navigationModel, selectedItem, -1),
		});
	const nextSection = (selectedItem: Item) =>
		dispatchProjectState({
			_tag: "SelectItem",
			item: getAdjacentSection(navigationModel, selectedItem, 1),
		});

	const handleItemSelectionAction = (action: ItemSelectionAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Move: ({ offset }) => move(offset, selectedItem),
				PreviousSection: () => previousSection(selectedItem),
				NextSection: () => nextSection(selectedItem),
			}),
		);

	const handlePrimaryPanelAction = (action: PrimaryPanelAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tags({
				FocusPreview: () => dispatchProjectState({ _tag: "FocusPreview" }),
				ToggleFullscreenPreview: () => dispatchProjectState({ _tag: "ToggleFullscreenPreview" }),
				TogglePreview: () => dispatchProjectState({ _tag: "TogglePreview" }),
			}),
			Match.orElse((action) => handleItemSelectionAction(action, selectedItem)),
		);

	const handleHunkSelectionAction = (action: HunkSelectionAction) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Move: ({ offset }) => previewRef.current?.moveSelection(offset),
			}),
		);

	const handlePreviewAction = (action: PreviewAction) =>
		Match.value(action).pipe(
			Match.tags({
				FocusPrimary: () => dispatchProjectState({ _tag: "FocusPrimary" }),
				ToggleFullscreenPreview: () => dispatchProjectState({ _tag: "ToggleFullscreenPreview" }),
				ClosePreview: () => dispatchProjectState({ _tag: "ClosePreview" }),
				TogglePreview: () => dispatchProjectState({ _tag: "TogglePreview" }),
			}),
			Match.orElse((action) => handleHunkSelectionAction(action)),
		);

	const handleChangesAction = (
		action: ChangesAction,
		selectedItem: ({ _tag: "Changes" } & ChangesSectionItem) | ({ _tag: "Change" } & ChangeItem),
	) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => requestAbsorptionPlanForSelection(selectedItem),
			}),
			Match.orElse((action) => handlePrimaryPanelAction(action, selectedItem)),
		);

	const handleCommitDefaultAction = (action: CommitDefaultAction, selectedItem: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				EditMessage: () =>
					dispatchProjectState({
						_tag: "SelectItem",
						item: commitItem({ ...selectedItem, mode: { _tag: "Reword" } }),
					}),
				OpenDetails: () => openCommitDetails(selectedItem),
			}),
			Match.orElse((action) =>
				handlePrimaryPanelAction(action, { _tag: "Commit", ...selectedItem }),
			),
		);

	const handleCommitDetailsAction = (action: CommitDetailsAction, selectedItem: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				Move: ({ offset }) => moveCommitDetailsFile(offset, selectedItem),
				CloseDetails: () =>
					dispatchProjectState({
						_tag: "SelectItem",
						item: commitItem({ ...selectedItem, mode: { _tag: "Default" } }),
					}),
			}),
			Match.orElse((action) =>
				handlePrimaryPanelAction(action, { _tag: "Commit", ...selectedItem }),
			),
		);

	const handleBranchAction = (action: BranchAction, selectedItem: SegmentItem) =>
		Match.value(action).pipe(
			Match.tags({
				RenameBranch: () =>
					dispatchProjectState({
						_tag: "SelectItem",
						item: {
							_tag: "Segment",
							...selectedItem,
							mode: { _tag: "Rename" },
						},
					}),
			}),
			Match.orElse((action) =>
				handlePrimaryPanelAction(action, { _tag: "Segment", ...selectedItem }),
			),
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
					handleChangesAction(action, { _tag: "Changes", ...scope.context });
				},
				Change: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangesAction(action, { _tag: "Change", ...scope.context });
				},
				BaseCommit: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handlePrimaryPanelAction(action, { _tag: "BaseCommit", ...scope.context });
				},
				Segment: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handlePrimaryPanelAction(action, { _tag: "Segment", ...scope.context });
				},
				BranchDefault: (scope) => {
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
				CommitDefault: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleCommitDefaultAction(action, scope.context);
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
