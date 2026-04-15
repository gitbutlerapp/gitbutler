import { getAction, type ShortcutActionBase, type ShortcutBinding } from "#ui/shortcuts.ts";
import { useRunOperation } from "#ui/Operation.ts";
import { isTypingTarget } from "#ui/routes/project/$id/shared.tsx";
import { getFocus, type ProjectLayoutState } from "#ui/routes/project/$id/state/layout.ts";
import { projectActions } from "#ui/routes/project/$id/state/projectSlice.ts";
import { useAppDispatch } from "#ui/state/hooks.ts";
import { AbsorptionTarget } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { RefObject, useEffect, useEffectEvent } from "react";
import {
	baseCommitItem,
	changeItem,
	commitFileItem,
	commitItem,
	type ChangeItem,
	changesSectionItem,
	type ChangesSectionItem,
	type CommitFileItem,
	type CommitItem,
	type Item,
	segmentItem,
	type SegmentItem,
	StackItem,
	stackItem,
} from "./Item.ts";
import { operationModeToOperation } from "./OperationMode.tsx";
import { operationSourceFromItem } from "./OperationSource.ts";
import { useResolveOperationSource } from "./ResolvedOperationSource.ts";
import { PreviewImperativeHandle } from "./route.tsx";
import {
	getAdjacentItem,
	getAdjacentSection,
	getPreviousSection,
	type NavigationIndex,
} from "./WorkspaceModel.ts";
import { OperationMode, type WorkspaceMode } from "./WorkspaceMode.ts";

type MoveItemSelectionAction = { offset: -1 | 1 };

type ItemSelectionAction =
	| { _tag: "EnterMoveMode" }
	| { _tag: "EnterRubMode" }
	| ({ _tag: "Move" } & MoveItemSelectionAction)
	| { _tag: "NextSection" }
	| { _tag: "PreviousSection" };

const enterMoveModeAction: ItemSelectionAction = { _tag: "EnterMoveMode" };
const enterRubModeAction: ItemSelectionAction = { _tag: "EnterRubMode" };
const moveItemSelectionAction = ({ offset }: MoveItemSelectionAction): ItemSelectionAction => ({
	_tag: "Move",
	offset,
});
const nextSectionAction: ItemSelectionAction = { _tag: "NextSection" };
const previousSectionAction: ItemSelectionAction = { _tag: "PreviousSection" };

const itemSelectionBindings: Array<ShortcutBinding<ItemSelectionAction>> = [
	{
		id: "item-selection-move-up",
		description: "up",
		keys: ["ArrowUp", "k"],
		action: moveItemSelectionAction({ offset: -1 }),
	},
	{
		id: "item-selection-move-down",
		description: "down",
		keys: ["ArrowDown", "j"],
		action: moveItemSelectionAction({ offset: 1 }),
	},
	{
		id: "item-selection-previous-section",
		description: "Previous section",
		keys: ["Shift+ArrowUp", "Shift+k"],
		action: previousSectionAction,
		showInShortcutsBar: false,
	},
	{
		id: "item-selection-next-section",
		description: "Next section",
		keys: ["Shift+ArrowDown", "Shift+j"],
		action: nextSectionAction,
		showInShortcutsBar: false,
	},
	{
		id: "item-selection-enter-rub-mode",
		description: "Rub",
		keys: ["r"],
		action: enterRubModeAction,
		repeat: false,
	},
	{
		id: "item-selection-enter-move-mode",
		description: "Move",
		keys: ["m"],
		action: enterMoveModeAction,
		repeat: false,
	},
];

type GlobalPreviewAction = { _tag: "ToggleFullscreenPreview" } | { _tag: "TogglePreview" };

const toggleFullscreenPreviewAction: GlobalPreviewAction = { _tag: "ToggleFullscreenPreview" };
const togglePreviewAction: GlobalPreviewAction = { _tag: "TogglePreview" };

export const togglePreviewBinding: ShortcutBinding<GlobalPreviewAction> = {
	id: "primary-panel-toggle-preview",
	description: "Preview",
	keys: ["p"],
	action: togglePreviewAction,
	repeat: false,
};

export const toggleFullscreenPreviewBinding: ShortcutBinding<GlobalPreviewAction> = {
	id: "primary-panel-toggle-fullscreen-preview",
	description: "Fullscreen preview",
	keys: ["d"],
	action: toggleFullscreenPreviewAction,
	repeat: false,
};

type PrimaryPanelAction =
	| ItemSelectionAction
	| { _tag: "Commit" }
	| { _tag: "FocusPreview" }
	| { _tag: "SelectUnassignedChanges" }
	| GlobalPreviewAction;

const commitAction: PrimaryPanelAction = { _tag: "Commit" };
const focusPreviewAction: PrimaryPanelAction = { _tag: "FocusPreview" };
const selectUnassignedChangesAction: PrimaryPanelAction = { _tag: "SelectUnassignedChanges" };

const primaryPanelBindings: Array<ShortcutBinding<PrimaryPanelAction>> = [
	...itemSelectionBindings,
	{
		id: "primary-panel-commit",
		description: "Commit",
		keys: ["c"],
		action: commitAction,
		repeat: false,
	},
	{
		id: "primary-panel-select-unassigned-changes",
		description: "Unassigned changes",
		keys: ["z"],
		action: selectUnassignedChangesAction,
		repeat: false,
	},
	{
		id: "primary-panel-focus-preview",
		description: "Focus preview",
		keys: ["l"],
		action: focusPreviewAction,
		repeat: false,
	},
	toggleFullscreenPreviewBinding,
	togglePreviewBinding,
];

type ChangesAction = PrimaryPanelAction | { _tag: "Absorb" };

const absorbAction: ChangesAction = { _tag: "Absorb" };

const changesBindings: Array<ShortcutBinding<ChangesAction>> = [
	...primaryPanelBindings,
	{
		id: "changes-absorb",
		description: "Absorb",
		keys: ["a"],
		action: absorbAction,
		repeat: false,
	},
];

type CommitToggleFilesAction = { _tag: "ToggleFiles" };

const toggleCommitFilesAction: CommitToggleFilesAction = { _tag: "ToggleFiles" };

const toggleCommitFilesBinding: ShortcutBinding<CommitToggleFilesAction> = {
	id: "commit-toggle-files",
	description: "Files",
	keys: ["f"],
	action: toggleCommitFilesAction,
	repeat: false,
};

type CommitAction = PrimaryPanelAction | CommitToggleFilesAction | { _tag: "EditMessage" };

const editMessageAction: CommitAction = { _tag: "EditMessage" };

const commitDefaultBindings: Array<ShortcutBinding<CommitAction>> = [
	...primaryPanelBindings,
	toggleCommitFilesBinding,
	{
		id: "commit-reword",
		description: "Reword",
		keys: ["Enter"],
		action: editMessageAction,
		repeat: false,
	},
];

type CommitFileAction = PrimaryPanelAction | CommitToggleFilesAction | { _tag: "CloseFiles" };

const closeFilesAction: CommitFileAction = { _tag: "CloseFiles" };

const commitFilesBindings: Array<ShortcutBinding<CommitFileAction>> = [
	...primaryPanelBindings,
	toggleCommitFilesBinding,
	{
		id: "commit-file-close",
		description: "Close",
		keys: ["Escape"],
		action: closeFilesAction,
		repeat: false,
	},
];

type BranchAction = PrimaryPanelAction | { _tag: "RenameBranch" };

const renameBranchAction: BranchAction = { _tag: "RenameBranch" };

const branchBindings: Array<ShortcutBinding<BranchAction>> = [
	...primaryPanelBindings,
	{
		id: "branch-rename",
		description: "Rename",
		keys: ["Enter"],
		action: renameBranchAction,
		repeat: false,
	},
];

type BaseCommitDefaultModeScope = {
	bindings: Array<ShortcutBinding<PrimaryPanelAction>>;
};
type BranchDefaultModeScope = {
	bindings: Array<ShortcutBinding<BranchAction>>;
	context: SegmentItem;
};
type ChangeDefaultModeScope = {
	bindings: Array<ShortcutBinding<ChangesAction>>;
	context: ChangeItem;
};
type ChangesSectionDefaultModeScope = {
	bindings: Array<ShortcutBinding<ChangesAction>>;
	context: ChangesSectionItem;
};
type CommitDefaultModeScope = {
	bindings: Array<ShortcutBinding<CommitAction>>;
	context: CommitItem;
};
type CommitFileDefaultModeScope = {
	bindings: Array<ShortcutBinding<CommitFileAction>>;
	context: CommitFileItem;
};
type SegmentDefaultModeScope = {
	bindings: Array<ShortcutBinding<PrimaryPanelAction>>;
	context: SegmentItem;
};

type StackDefaultModeScope = {
	bindings: Array<ShortcutBinding<PrimaryPanelAction>>;
	context: StackItem;
};

type DefaultModeScope =
	| ({ _tag: "BaseCommit" } & BaseCommitDefaultModeScope)
	| ({ _tag: "Branch" } & BranchDefaultModeScope)
	| ({ _tag: "Change" } & ChangeDefaultModeScope)
	| ({ _tag: "ChangesSection" } & ChangesSectionDefaultModeScope)
	| ({ _tag: "Commit" } & CommitDefaultModeScope)
	| ({ _tag: "CommitFile" } & CommitFileDefaultModeScope)
	| ({ _tag: "Stack" } & StackDefaultModeScope)
	| ({ _tag: "Segment" } & SegmentDefaultModeScope);

const baseCommitDefaultModeScope = ({
	bindings,
}: BaseCommitDefaultModeScope): DefaultModeScope => ({
	_tag: "BaseCommit",
	bindings,
});

const branchDefaultModeScope = ({
	bindings,
	context,
}: BranchDefaultModeScope): DefaultModeScope => ({
	_tag: "Branch",
	bindings,
	context,
});

const changeDefaultModeScope = ({
	bindings,
	context,
}: ChangeDefaultModeScope): DefaultModeScope => ({
	_tag: "Change",
	bindings,
	context,
});

const changesSectionDefaultModeScope = ({
	bindings,
	context,
}: ChangesSectionDefaultModeScope): DefaultModeScope => ({
	_tag: "ChangesSection",
	bindings,
	context,
});

const commitDefaultModeScope = ({
	bindings,
	context,
}: CommitDefaultModeScope): DefaultModeScope => ({
	_tag: "Commit",
	bindings,
	context,
});

const commitFileDefaultModeScope = ({
	bindings,
	context,
}: CommitFileDefaultModeScope): DefaultModeScope => ({
	_tag: "CommitFile",
	bindings,
	context,
});

const stackDefaultModeScope = ({ bindings, context }: StackDefaultModeScope): DefaultModeScope => ({
	_tag: "Stack",
	bindings,
	context,
});

const segmentDefaultModeScope = ({
	bindings,
	context,
}: SegmentDefaultModeScope): DefaultModeScope => ({
	_tag: "Segment",
	bindings,
	context,
});

const getDefaultModeScope = (selectedItem: Item): DefaultModeScope =>
	Match.value(selectedItem).pipe(
		Match.tagsExhaustive({
			BaseCommit: () =>
				baseCommitDefaultModeScope({
					bindings: primaryPanelBindings,
				}),
			Change: (selectedItem) =>
				changeDefaultModeScope({
					bindings: changesBindings,
					context: selectedItem,
				}),
			ChangesSection: (selectedItem) =>
				changesSectionDefaultModeScope({
					bindings: changesBindings,
					context: selectedItem,
				}),
			Commit: (selectedItem) =>
				commitDefaultModeScope({
					bindings: commitDefaultBindings,
					context: selectedItem,
				}),
			CommitFile: (selectedItem) =>
				commitFileDefaultModeScope({
					bindings: commitFilesBindings,
					context: selectedItem,
				}),
			Stack: (selectedItem) =>
				stackDefaultModeScope({
					bindings: primaryPanelBindings,
					context: selectedItem,
				}),
			Segment: (selectedItem) =>
				selectedItem.branchRef === null
					? segmentDefaultModeScope({
							bindings: primaryPanelBindings,
							context: selectedItem,
						})
					: branchDefaultModeScope({
							bindings: branchBindings,
							context: selectedItem,
						}),
		}),
	);

const getDefaultModeScopeLabel = (scope: DefaultModeScope): string =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => "Base commit",
			Branch: () => "Branch",
			Change: () => "Change",
			ChangesSection: () => "Changes",
			Commit: () => "Commit",
			CommitFile: () => "Commit file",
			Stack: () => "Stack",
			Segment: () => "Segment",
		}),
	);

type HunkSelectionAction = { offset: -1 | 1 };

type PreviewAction =
	| { _tag: "ClosePreview" }
	| { _tag: "FocusPrimary" }
	| ({ _tag: "Move" } & HunkSelectionAction)
	| GlobalPreviewAction;

const closePreviewAction: PreviewAction = { _tag: "ClosePreview" };
const focusPrimaryAction: PreviewAction = { _tag: "FocusPrimary" };
const moveHunkSelectionAction = ({ offset }: HunkSelectionAction): PreviewAction => ({
	_tag: "Move",
	offset,
});

export const closePreviewBinding: ShortcutBinding<PreviewAction> = {
	id: "preview-close",
	description: "Close",
	keys: ["Escape"],
	action: closePreviewAction,
	repeat: false,
};

const previewBindings: Array<ShortcutBinding<PreviewAction>> = [
	{
		id: "preview-move-up",
		description: "up",
		keys: ["ArrowUp", "k"],
		action: moveHunkSelectionAction({ offset: -1 }),
	},
	{
		id: "preview-move-down",
		description: "down",
		keys: ["ArrowDown", "j"],
		action: moveHunkSelectionAction({ offset: 1 }),
	},
	{
		id: "preview-focus-primary",
		description: "Focus primary",
		keys: ["h"],
		action: focusPrimaryAction,
		repeat: false,
	},
	toggleFullscreenPreviewBinding,
	togglePreviewBinding,
	closePreviewBinding,
];

const fullscreenPreviewBindings: Array<ShortcutBinding<PreviewAction>> = previewBindings
	// The preview panel is not visible as it sits behind the fullscreen dialog, so
	// there's no point having the toggle preview shortcut here.
	.filter((binding) => binding.action._tag !== "TogglePreview");

type OperationModeAction = PrimaryPanelAction | { _tag: "Cancel" } | { _tag: "Run" };

const runOperationModeAction: OperationModeAction = { _tag: "Run" };
const cancelOperationModeAction: OperationModeAction = { _tag: "Cancel" };

const operationModeBindings: Array<ShortcutBinding<OperationModeAction>> = [
	...primaryPanelBindings.filter(
		(binding) =>
			binding.action._tag !== "Commit" &&
			binding.action._tag !== "EnterRubMode" &&
			binding.action._tag !== "EnterMoveMode",
	),
	{
		id: "operation-mode-confirm",
		description: "Run",
		keys: ["Enter"],
		action: runOperationModeAction,
		repeat: false,
	},
	{
		id: "operation-mode-cancel",
		description: "Cancel",
		keys: ["Escape"],
		action: cancelOperationModeAction,
		repeat: false,
	},
];

type MoveOperationModeScope = {
	bindings: Array<ShortcutBinding<OperationModeAction>>;
	context: Item | null;
};
type RubOperationModeScope = {
	bindings: Array<ShortcutBinding<OperationModeAction>>;
	context: Item | null;
};

type OperationModeScope =
	| ({ _tag: "Move" } & MoveOperationModeScope)
	| ({ _tag: "Rub" } & RubOperationModeScope);

const moveOperationModeScope = ({
	bindings,
	context,
}: MoveOperationModeScope): OperationModeScope => ({
	_tag: "Move",
	bindings,
	context,
});

const rubOperationModeScope = ({
	bindings,
	context,
}: RubOperationModeScope): OperationModeScope => ({
	_tag: "Rub",
	bindings,
	context,
});

const getOperationModeScopeLabel = (scope: OperationModeScope): string =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			Move: () => "Move mode",
			Rub: () => "Rub mode",
		}),
	);

type RewordCommitAction = { _tag: "Cancel" } | { _tag: "Save" };

const saveRewordCommitAction: RewordCommitAction = { _tag: "Save" };
const cancelRewordCommitAction: RewordCommitAction = { _tag: "Cancel" };

export const rewordCommitBindings: Array<ShortcutBinding<RewordCommitAction>> = [
	{
		id: "commit-reword-save",
		description: "Save",
		keys: ["Enter"],
		action: saveRewordCommitAction,
		repeat: false,
	},
	{
		id: "commit-reword-cancel",
		description: "Cancel",
		keys: ["Escape"],
		action: cancelRewordCommitAction,
		repeat: false,
	},
];

type RenameBranchAction = { _tag: "Cancel" } | { _tag: "Save" };

const saveRenameBranchAction: RenameBranchAction = { _tag: "Save" };
const cancelRenameBranchAction: RenameBranchAction = { _tag: "Cancel" };

export const renameBranchBindings: Array<ShortcutBinding<RenameBranchAction>> = [
	{
		id: "branch-rename-save",
		description: "Save",
		keys: ["Enter"],
		action: saveRenameBranchAction,
		repeat: false,
	},
	{
		id: "branch-rename-cancel",
		description: "Cancel",
		keys: ["Escape"],
		action: cancelRenameBranchAction,
		repeat: false,
	},
];

type DefaultModeScopeContainer = { scope: DefaultModeScope };
type RenameBranchModeScope = {
	bindings: Array<ShortcutBinding<RenameBranchAction>>;
	context: SegmentItem;
};
type RewordCommitModeScope = {
	bindings: Array<ShortcutBinding<RewordCommitAction>>;
	context: CommitItem;
};

type ModeScope =
	| ({ _tag: "Default" } & DefaultModeScopeContainer)
	| ({ _tag: "RenameBranch" } & RenameBranchModeScope)
	| ({ _tag: "RewordCommit" } & RewordCommitModeScope)
	| OperationModeScope;

const defaultModeScope = ({ scope }: DefaultModeScopeContainer): ModeScope => ({
	_tag: "Default",
	scope,
});

const renameBranchModeScope = ({ bindings, context }: RenameBranchModeScope): ModeScope => ({
	_tag: "RenameBranch",
	bindings,
	context,
});

const rewordCommitModeScope = ({ bindings, context }: RewordCommitModeScope): ModeScope => ({
	_tag: "RewordCommit",
	bindings,
	context,
});

const getModeScope = ({
	selectedItem,
	workspaceMode,
}: {
	selectedItem: Item | null;
	workspaceMode: WorkspaceMode;
}): ModeScope | null =>
	Match.value(workspaceMode).pipe(
		Match.tagsExhaustive({
			Default: () =>
				selectedItem
					? defaultModeScope({
							scope: getDefaultModeScope(selectedItem),
						})
					: null,
			Move: () =>
				moveOperationModeScope({
					bindings: operationModeBindings,
					context: selectedItem,
				}),
			RenameBranch: (workspaceMode) =>
				selectedItem?._tag === "Segment" &&
				workspaceMode.stackId === selectedItem.stackId &&
				workspaceMode.segmentIndex === selectedItem.segmentIndex
					? renameBranchModeScope({
							bindings: renameBranchBindings,
							context: selectedItem,
						})
					: null,
			RewordCommit: (workspaceMode) =>
				selectedItem?._tag === "Commit" && workspaceMode.commitId === selectedItem.commitId
					? rewordCommitModeScope({
							bindings: rewordCommitBindings,
							context: selectedItem,
						})
					: null,
			Rub: () =>
				rubOperationModeScope({
					bindings: operationModeBindings,
					context: selectedItem,
				}),
		}),
	);

type PreviewScope = {
	bindings: Array<ShortcutBinding<PreviewAction>>;
	context: { isFullscreen: boolean };
};

type Scope = ModeScope | ({ _tag: "Preview" } & PreviewScope);

const isOperationModeScope = (scope: Scope): scope is OperationModeScope =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			Move: () => true,
			Rub: () => true,
			Default: () => false,
			RenameBranch: () => false,
			RewordCommit: () => false,
			Preview: () => false,
		}),
	);

const previewScope = ({ bindings, context }: PreviewScope): Scope => ({
	_tag: "Preview",
	bindings,
	context,
});

export const getScope = ({
	selectedItem,
	layoutState,
	workspaceMode,
}: {
	selectedItem: Item | null;
	layoutState: ProjectLayoutState;
	workspaceMode: WorkspaceMode;
}): Scope | null => {
	if (getFocus(layoutState) === "preview")
		return previewScope({
			bindings: layoutState.isFullscreenPreviewOpen ? fullscreenPreviewBindings : previewBindings,
			context: { isFullscreen: layoutState.isFullscreenPreviewOpen },
		});

	return getModeScope({ selectedItem, workspaceMode });
};

export const getScopeBindings = (scope: Scope): Array<ShortcutBinding<ShortcutActionBase>> =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			Default: ({ scope }) => scope.bindings,
			Move: ({ bindings }) => bindings,
			Preview: ({ bindings }) => bindings,
			RenameBranch: ({ bindings }) => bindings,
			RewordCommit: ({ bindings }) => bindings,
			Rub: ({ bindings }) => bindings,
		}),
	);

export const getScopeLabel = (scope: Scope): string =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			Default: ({ scope }) => getDefaultModeScopeLabel(scope),
			Move: (scope) => getOperationModeScopeLabel(scope),
			Preview: () => "Preview",
			RenameBranch: () => "Rename branch",
			RewordCommit: () => "Reword commit",
			Rub: (scope) => getOperationModeScopeLabel(scope),
		}),
	);

export const useWorkspaceShortcuts = ({
	branchRenameFormRef,
	commitMessageFormRef,
	projectId,
	scope,
	navigationIndex,
	requestAbsorptionPlan,
	operationMode,
	previewRef,
}: {
	branchRenameFormRef: RefObject<HTMLFormElement | null>;
	commitMessageFormRef: RefObject<HTMLFormElement | null>;
	projectId: string;
	scope: Scope | null;
	navigationIndex: NavigationIndex;
	requestAbsorptionPlan: (target: AbsorptionTarget) => void;
	operationMode: OperationMode | null;
	previewRef: RefObject<PreviewImperativeHandle | null>;
}) => {
	const dispatch = useAppDispatch();
	const resolveOperationSource = useResolveOperationSource(projectId);
	const runOperation = useRunOperation();

	const resolvedOperationModeSource = operationMode
		? resolveOperationSource(operationMode.source)
		: null;

	const selectItem = (item: Item | null) =>
		dispatch(
			projectActions.selectItem({
				projectId,
				item,
			}),
		);

	const selectRelativeItem = (selectedItem: Item, offset: -1 | 1) =>
		selectItem(getAdjacentItem(navigationIndex, selectedItem, offset) ?? null);

	const selectPreviousSectionItem = (selectedItem: Item) =>
		selectItem(getPreviousSection(navigationIndex, selectedItem));

	const selectNextSectionItem = (selectedItem: Item) =>
		selectItem(getAdjacentSection(navigationIndex, selectedItem, 1) ?? null);

	const handleItemSelectionAction = (action: ItemSelectionAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				EnterMoveMode: () =>
					dispatch(
						projectActions.enterMoveMode({
							projectId,
							source: operationSourceFromItem(selectedItem),
						}),
					),
				EnterRubMode: () =>
					dispatch(
						projectActions.enterRubMode({
							projectId,
							source: operationSourceFromItem(selectedItem),
						}),
					),
				Move: ({ offset }) => selectRelativeItem(selectedItem, offset),
				NextSection: () => selectNextSectionItem(selectedItem),
				PreviousSection: () => selectPreviousSectionItem(selectedItem),
			}),
		);

	const handlePrimaryPanelAction = (action: PrimaryPanelAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tags({
				Commit: () =>
					dispatch(
						projectActions.enterMoveMode({
							projectId,
							source: operationSourceFromItem(changesSectionItem({ stackId: null })),
						}),
					),
				FocusPreview: () => dispatch(projectActions.focusPreview({ projectId })),
				SelectUnassignedChanges: () => selectItem(changesSectionItem({ stackId: null })),
				ToggleFullscreenPreview: () =>
					dispatch(projectActions.toggleFullscreenPreview({ projectId })),
				TogglePreview: () => dispatch(projectActions.togglePreview({ projectId })),
			}),
			Match.orElse((action) => handleItemSelectionAction(action, selectedItem)),
		);

	const requestAbsorptionPlanForItem = (selectedItem: Item) => {
		const operationSource = operationSourceFromItem(selectedItem);
		const resolvedOperationSource = resolveOperationSource(operationSource);

		if (resolvedOperationSource?._tag !== "TreeChanges") return;
		if (resolvedOperationSource.parent._tag !== "ChangesSection") return;

		requestAbsorptionPlan({
			type: "treeChanges",
			subject: {
				changes: resolvedOperationSource.changes.map(({ change }) => change),
				assignedStackId: null,
			},
		});
	};

	const handleChangesScopeAction = (action: ChangesAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => requestAbsorptionPlanForItem(selectedItem),
			}),
			Match.orElse((action) => handlePrimaryPanelAction(action, selectedItem)),
		);

	const handleCommitScopeAction = (action: CommitAction, selectedItem: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				EditMessage: () =>
					dispatch(
						projectActions.startRewordCommit({
							projectId,
							item: selectedItem,
						}),
					),
				ToggleFiles: () =>
					dispatch(projectActions.toggleCommitFiles({ projectId, item: selectedItem })),
			}),
			Match.orElse((action) => handlePrimaryPanelAction(action, commitItem(selectedItem))),
		);

	const handleCommitFileScopeAction = (action: CommitFileAction, selectedItem: CommitFileItem) =>
		Match.value(action).pipe(
			Match.tags({
				CloseFiles: () =>
					dispatch(projectActions.closeCommitFiles({ projectId, item: selectedItem })),
				ToggleFiles: () =>
					dispatch(projectActions.toggleCommitFiles({ projectId, item: selectedItem })),
			}),
			Match.orElse((action) => handlePrimaryPanelAction(action, commitFileItem(selectedItem))),
		);

	const handleBranchScopeAction = (action: BranchAction, selectedItem: SegmentItem) =>
		Match.value(action).pipe(
			Match.tags({
				RenameBranch: () =>
					dispatch(
						projectActions.startRenameBranch({
							projectId,
							item: selectedItem,
						}),
					),
			}),
			Match.orElse((action) => handlePrimaryPanelAction(action, segmentItem(selectedItem))),
		);

	const handleDefaultScopeKeyDown = (scope: DefaultModeScope, event: KeyboardEvent) =>
		Match.value(scope).pipe(
			Match.tagsExhaustive({
				BaseCommit: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handlePrimaryPanelAction(action, baseCommitItem);
				},
				Branch: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleBranchScopeAction(action, scope.context);
				},
				Change: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangesScopeAction(action, changeItem(scope.context));
				},
				ChangesSection: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangesScopeAction(action, changesSectionItem(scope.context));
				},
				Commit: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleCommitScopeAction(action, scope.context);
				},
				CommitFile: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleCommitFileScopeAction(action, scope.context);
				},
				Stack: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handlePrimaryPanelAction(action, stackItem(scope.context));
				},
				Segment: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handlePrimaryPanelAction(action, segmentItem(scope.context));
				},
			}),
		);

	const handlePreviewScopeAction = (action: PreviewAction) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				ClosePreview: () => dispatch(projectActions.closePreview({ projectId })),
				FocusPrimary: () => dispatch(projectActions.focusPrimary({ projectId })),
				Move: ({ offset }) => previewRef.current?.moveSelection(offset),
				ToggleFullscreenPreview: () =>
					dispatch(projectActions.toggleFullscreenPreview({ projectId })),
				TogglePreview: () => dispatch(projectActions.togglePreview({ projectId })),
			}),
		);

	const confirmOperationMode = (selectedItem: Item | null) => {
		dispatch(projectActions.exitMode({ projectId }));

		const operationModeOperation =
			operationMode && selectedItem && resolvedOperationModeSource
				? operationModeToOperation({
						operationMode,
						resolvedOperationSource: resolvedOperationModeSource,
						target: selectedItem,
					})
				: null;

		if (!operationModeOperation) return;

		runOperation(projectId, operationModeOperation);
	};

	const handleOperationModeScopeAction = (action: OperationModeAction, selectedItem: Item | null) =>
		Match.value(action).pipe(
			Match.tags({
				Cancel: () => dispatch(projectActions.exitMode({ projectId })),
				Run: () => confirmOperationMode(selectedItem),
			}),
			Match.orElse((action) => {
				if (!selectedItem) return;
				handlePrimaryPanelAction(action, selectedItem);
			}),
		);

	const handleOperationModeScopeKeyDown = (scope: OperationModeScope, event: KeyboardEvent) => {
		const action = getAction(scope.bindings, event);
		if (!action) return;
		event.preventDefault();
		handleOperationModeScopeAction(action, scope.context);
	};

	const handleRewordCommitScopeAction = (action: RewordCommitAction) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Cancel: () => dispatch(projectActions.exitMode({ projectId })),
				Save: () => commitMessageFormRef.current?.requestSubmit(),
			}),
		);

	const handleRenameBranchScopeAction = (action: RenameBranchAction) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Cancel: () => dispatch(projectActions.exitMode({ projectId })),
				Save: () => branchRenameFormRef.current?.requestSubmit(),
			}),
		);

	const handleScopeKeyDown = (scope: Scope, event: KeyboardEvent) =>
		isOperationModeScope(scope)
			? handleOperationModeScopeKeyDown(scope, event)
			: Match.value(scope).pipe(
					Match.tagsExhaustive({
						Default: ({ scope }) => handleDefaultScopeKeyDown(scope, event),
						Preview: (scope) => {
							const action = getAction(scope.bindings, event);
							if (!action) return;
							event.preventDefault();
							handlePreviewScopeAction(action);
						},
						RenameBranch: (scope) => {
							const action = getAction(scope.bindings, event);
							if (!action) return;
							event.preventDefault();
							handleRenameBranchScopeAction(action);
						},
						RewordCommit: (scope) => {
							const action = getAction(scope.bindings, event);
							if (!action) return;
							event.preventDefault();
							handleRewordCommitScopeAction(action);
						},
					}),
				);

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented) return;
		if (!scope) return;
		if (
			scope._tag !== "RewordCommit" &&
			scope._tag !== "RenameBranch" &&
			isTypingTarget(event.target)
		)
			return;

		handleScopeKeyDown(scope, event);
	});

	useEffect(() => {
		window.addEventListener("keydown", handleKeyDown);

		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, []);
};
