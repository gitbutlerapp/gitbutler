import { getAction, type ShortcutActionBase, type ShortcutBinding } from "#ui/shortcuts.ts";
import { getOperation, useRunOperation } from "#ui/Operation.ts";
import { type Panel } from "#ui/routes/project/$id/state/layout.ts";
import {
	projectActions,
	selectProjectOperationModeState,
} from "#ui/routes/project/$id/state/projectSlice.ts";
import { useAppDispatch, useAppSelector } from "#ui/state/hooks.ts";
import { Match } from "effect";
import { RefObject, useEffect, useEffectEvent } from "react";
import {
	branchItem,
	baseCommitItem,
	commitItem,
	fileItem,
	itemEquals,
	type BranchItem,
	changesSectionItem,
	type CommitItem,
	type FileItem,
	type Item,
	StackItem,
	stackItem,
} from "./Item.ts";
import { resolveAbsorptionTarget } from "./Absorption.tsx";
import {
	getAdjacent,
	getNextSection,
	getPreviousSection,
	type NavigationIndex,
} from "./WorkspaceModel.ts";
import { operationModeToOperationType, type WorkspaceMode } from "./WorkspaceMode.ts";
import { useQueryClient } from "@tanstack/react-query";
import { AbsorptionTarget } from "@gitbutler/but-sdk";
import { changesInWorktreeQueryOptions } from "#ui/api/queries.ts";

const isTypingTarget = (target: EventTarget | null) => {
	if (!(target instanceof HTMLElement)) return false;
	return (
		target.isContentEditable ||
		target instanceof HTMLInputElement ||
		target instanceof HTMLTextAreaElement
	);
};

type MoveItemSelectionAction = { offset: -1 | 1 };

type ItemMoveSelectionAction =
	| ({ _tag: "Move" } & MoveItemSelectionAction)
	| { _tag: "NextSection" }
	| { _tag: "PreviousSection" };

const moveItemSelectionAction = ({ offset }: MoveItemSelectionAction): ItemMoveSelectionAction => ({
	_tag: "Move",
	offset,
});
const nextSectionAction: ItemMoveSelectionAction = { _tag: "NextSection" };
const previousSectionAction: ItemMoveSelectionAction = { _tag: "PreviousSection" };

const itemMoveSelectionBindings: Array<ShortcutBinding<ItemMoveSelectionAction>> = [
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
];

type ItemSelectionAction =
	| { _tag: "EnterMoveMode" }
	| { _tag: "EnterRubMode" }
	| ItemMoveSelectionAction;

const enterMoveModeAction: ItemSelectionAction = { _tag: "EnterMoveMode" };
const enterRubModeAction: ItemSelectionAction = { _tag: "EnterRubMode" };

const itemSelectionBindings: Array<ShortcutBinding<ItemSelectionAction>> = [
	...itemMoveSelectionBindings,
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

type PanelNavigationAction =
	| { _tag: "FocusPreviousPanel" }
	| { _tag: "FocusNextPanel" }
	| { _tag: "ToggleShow" };

const isPanelNavigationAction = (action: { _tag: string }): action is PanelNavigationAction =>
	action._tag === "FocusPreviousPanel" ||
	action._tag === "FocusNextPanel" ||
	action._tag === "ToggleShow";

const focusPreviousPanelAction: PanelNavigationAction = { _tag: "FocusPreviousPanel" };
const focusNextPanelAction: PanelNavigationAction = { _tag: "FocusNextPanel" };
const toggleShowAction: PanelNavigationAction = { _tag: "ToggleShow" };

export const toggleShowBinding: ShortcutBinding<PanelNavigationAction> = {
	id: "primary-panel-toggle-show",
	description: "Show",
	keys: ["p"],
	action: toggleShowAction,
	repeat: false,
};

const panelNavigationBindings: Array<ShortcutBinding<PanelNavigationAction>> = [
	{
		id: "panel-focus-left",
		description: "Focus previous panel",
		keys: ["h"],
		action: focusPreviousPanelAction,
		repeat: false,
	},
	{
		id: "panel-focus-right",
		description: "Focus next panel",
		keys: ["l"],
		action: focusNextPanelAction,
		repeat: false,
	},
	toggleShowBinding,
];

type PrimaryPanelAction =
	| ItemSelectionAction
	| { _tag: "Commit" }
	| { _tag: "SelectBranch" }
	| { _tag: "SelectChanges" }
	| PanelNavigationAction;

const commitAction: PrimaryPanelAction = { _tag: "Commit" };
const selectBranchAction: PrimaryPanelAction = { _tag: "SelectBranch" };
const selectChangesAction: PrimaryPanelAction = { _tag: "SelectChanges" };

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
		id: "primary-panel-select-changes",
		description: "Changes",
		keys: ["z"],
		action: selectChangesAction,
		repeat: false,
	},
	{
		id: "primary-panel-select-branch",
		description: "Branch",
		keys: ["t"],
		action: selectBranchAction,
		repeat: false,
	},
	...panelNavigationBindings,
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
	context: BranchItem;
};
type ChangesFileDefaultModeScope = {
	bindings: Array<ShortcutBinding<ChangesAction>>;
	context: FileItem;
};
type ChangesSectionDefaultModeScope = {
	bindings: Array<ShortcutBinding<ChangesAction>>;
};
type CommitDefaultModeScope = {
	bindings: Array<ShortcutBinding<CommitAction>>;
	context: CommitItem;
};
type CommitFileDefaultModeScope = {
	bindings: Array<ShortcutBinding<CommitFileAction>>;
	context: FileItem;
};

type StackDefaultModeScope = {
	bindings: Array<ShortcutBinding<PrimaryPanelAction>>;
	context: StackItem;
};

type DefaultModeScope =
	| ({ _tag: "BaseCommit" } & BaseCommitDefaultModeScope)
	| ({ _tag: "Branch" } & BranchDefaultModeScope)
	| ({ _tag: "ChangesFile" } & ChangesFileDefaultModeScope)
	| ({ _tag: "ChangesSection" } & ChangesSectionDefaultModeScope)
	| ({ _tag: "Commit" } & CommitDefaultModeScope)
	| ({ _tag: "CommitFile" } & CommitFileDefaultModeScope)
	| ({ _tag: "Stack" } & StackDefaultModeScope);

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

const changesFileDefaultModeScope = ({
	bindings,
	context,
}: ChangesFileDefaultModeScope): DefaultModeScope => ({
	_tag: "ChangesFile",
	bindings,
	context,
});

const changesSectionDefaultModeScope = ({
	bindings,
}: ChangesSectionDefaultModeScope): DefaultModeScope => ({
	_tag: "ChangesSection",
	bindings,
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

const getDefaultModeScope = (selectedItem: Item): DefaultModeScope | null =>
	Match.value(selectedItem).pipe(
		Match.tagsExhaustive({
			BaseCommit: () =>
				baseCommitDefaultModeScope({
					bindings: primaryPanelBindings,
				}),
			File: (selectedItem) =>
				Match.value(selectedItem.parent).pipe(
					Match.tagsExhaustive({
						Changes: () =>
							changesFileDefaultModeScope({
								bindings: changesBindings,
								context: selectedItem,
							}),
						Commit: () =>
							commitFileDefaultModeScope({
								bindings: commitFilesBindings,
								context: selectedItem,
							}),
						Branch: () => null,
					}),
				),
			ChangesSection: () =>
				changesSectionDefaultModeScope({
					bindings: changesBindings,
				}),
			Commit: (selectedItem) =>
				commitDefaultModeScope({
					bindings: commitDefaultBindings,
					context: selectedItem,
				}),
			Stack: (selectedItem) =>
				stackDefaultModeScope({
					bindings: primaryPanelBindings,
					context: selectedItem,
				}),
			Branch: (selectedItem) =>
				branchDefaultModeScope({
					bindings: branchBindings,
					context: selectedItem,
				}),
			Hunk: () => null,
		}),
	);

const getDefaultModeScopeLabel = (scope: DefaultModeScope): string =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => "Base commit",
			Branch: () => "Branch",
			ChangesFile: () => "Change",
			ChangesSection: () => "Changes",
			Commit: () => "Commit",
			CommitFile: () => "Commit file",
			Stack: () => "Stack",
		}),
	);

type ShowAction = { _tag: "CloseShow" } | PanelNavigationAction;

const closeShowAction: ShowAction = { _tag: "CloseShow" };

const closeShowBinding: ShortcutBinding<ShowAction> = {
	id: "show-close",
	description: "Close",
	keys: ["Escape"],
	action: closeShowAction,
	repeat: false,
};

const showBindings: Array<ShortcutBinding<ShowAction>> = [
	closeShowBinding,
	...panelNavigationBindings,
];

type OperationModeAction = PrimaryPanelAction | { _tag: "Cancel" } | { _tag: "Confirm" };

const confirmOperationModeAction: OperationModeAction = { _tag: "Confirm" };
const cancelOperationModeAction: OperationModeAction = { _tag: "Cancel" };

const operationModeBindings: Array<ShortcutBinding<OperationModeAction>> = [
	...primaryPanelBindings.filter(
		(binding) =>
			binding.action._tag !== "Commit" &&
			binding.action._tag !== "EnterRubMode" &&
			binding.action._tag !== "EnterMoveMode" &&
			binding.action._tag !== "SelectBranch",
	),
	{
		id: "operation-mode-confirm",
		description: "Confirm",
		keys: ["Enter"],
		action: confirmOperationModeAction,
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
	context: BranchItem;
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
	selectedItem: Item;
	workspaceMode: WorkspaceMode;
}): ModeScope | null =>
	Match.value(workspaceMode).pipe(
		Match.tagsExhaustive({
			Default: () => {
				const scope = getDefaultModeScope(selectedItem);
				if (!scope) return null;
				return defaultModeScope({
					scope,
				});
			},
			Operation: ({ value }) =>
				Match.value(value).pipe(
					Match.tagsExhaustive({
						DragAndDrop: () => null,
						Move: () =>
							moveOperationModeScope({
								bindings: operationModeBindings,
								context: selectedItem,
							}),
						Rub: () =>
							rubOperationModeScope({
								bindings: operationModeBindings,
								context: selectedItem,
							}),
					}),
				),
			RenameBranch: (workspaceMode) =>
				selectedItem._tag === "Branch" &&
				itemEquals(
					selectedItem,
					branchItem({
						stackId: workspaceMode.stackId,
						branchRef: workspaceMode.branchRef,
					}),
				)
					? renameBranchModeScope({
							bindings: renameBranchBindings,
							context: selectedItem,
						})
					: null,
			RewordCommit: (workspaceMode) =>
				selectedItem._tag === "Commit" &&
				itemEquals(
					selectedItem,
					commitItem({
						stackId: workspaceMode.stackId,
						commitId: workspaceMode.commitId,
					}),
				)
					? rewordCommitModeScope({
							bindings: rewordCommitBindings,
							context: selectedItem,
						})
					: null,
		}),
	);

type ShowScope = {
	bindings: Array<ShortcutBinding<ShowAction>>;
};

type Scope = ModeScope | ({ _tag: "Show" } & ShowScope);

const isOperationModeScope = (scope: Scope): scope is OperationModeScope =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			Move: () => true,
			Rub: () => true,
			Default: () => false,
			RenameBranch: () => false,
			RewordCommit: () => false,
			Show: () => false,
		}),
	);

const showScope = ({ bindings }: ShowScope): Scope => ({
	_tag: "Show",
	bindings,
});

export const getScope = ({
	selectedItem,
	focusedPanel,
	workspaceMode,
}: {
	selectedItem: Item;
	focusedPanel: Panel | null;
	workspaceMode: WorkspaceMode;
}): Scope | null =>
	Match.value(focusedPanel).pipe(
		Match.when(null, () => null),
		Match.when("show", () => showScope({ bindings: showBindings })),
		Match.when("primary", () => getModeScope({ selectedItem, workspaceMode })),
		Match.exhaustive,
	);

export const getScopeBindings = (scope: Scope): Array<ShortcutBinding<ShortcutActionBase>> =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			Default: ({ scope }) => scope.bindings,
			Move: ({ bindings }) => bindings,
			Show: ({ bindings }) => bindings,
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
			Show: () => "Preview (show)",
			RenameBranch: () => "Rename branch",
			RewordCommit: () => "Reword commit",
			Rub: (scope) => getOperationModeScopeLabel(scope),
		}),
	);

export const useWorkspaceShortcuts = ({
	inlineRenameBranchFormRef,
	inlineRewordCommitFormRef,
	projectId,
	scope,
	navigationIndex,
	openAbsorptionDialog,
	openBranchPicker,
	focusPanel,
	focusAdjacentPanel,
}: {
	inlineRenameBranchFormRef: RefObject<HTMLFormElement | null>;
	inlineRewordCommitFormRef: RefObject<HTMLFormElement | null>;
	projectId: string;
	scope: Scope | null;
	navigationIndex: NavigationIndex;
	openAbsorptionDialog: (target: AbsorptionTarget) => void;
	openBranchPicker: () => void;
	focusPanel: (panel: Panel) => void;
	focusAdjacentPanel: (offset: -1 | 1) => void;
}) => {
	const dispatch = useAppDispatch();
	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);
	const queryClient = useQueryClient();
	const runOperation = useRunOperation();

	const handleItemMoveSelectionAction = (action: ItemMoveSelectionAction, selectedItem: Item) => {
		const newItem = Match.value(action).pipe(
			Match.tagsExhaustive({
				Move: ({ offset }) =>
					getAdjacent({
						navigationIndex,
						selectedItem,
						offset,
					}),
				NextSection: () =>
					getNextSection({
						navigationIndex,
						selectedItem,
					}),
				PreviousSection: () =>
					getPreviousSection({
						navigationIndex,
						selectedItem,
					}),
			}),
		);
		if (!newItem) return;
		dispatch(projectActions.selectItem({ projectId, item: newItem }));
	};

	const handleItemSelectionAction = (action: ItemSelectionAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tags({
				EnterMoveMode: () =>
					dispatch(
						projectActions.enterMoveMode({
							projectId,
							source: selectedItem,
						}),
					),
				EnterRubMode: () =>
					dispatch(
						projectActions.enterRubMode({
							projectId,
							source: selectedItem,
						}),
					),
			}),
			Match.orElse((action) => {
				handleItemMoveSelectionAction(action, selectedItem);
			}),
		);

	const handlePanelNavigationAction = (action: PanelNavigationAction) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				FocusNextPanel: () => focusAdjacentPanel(1),
				FocusPreviousPanel: () => focusAdjacentPanel(-1),
				ToggleShow: () => dispatch(projectActions.togglePanel({ projectId, panel: "show" })),
			}),
		);

	const handlePrimaryPanelAction = (action: PrimaryPanelAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tags({
				Commit: () =>
					dispatch(
						projectActions.enterMoveMode({
							projectId,
							source: changesSectionItem,
						}),
					),
				SelectBranch: () => openBranchPicker(),
				SelectChanges: () =>
					dispatch(projectActions.selectItem({ projectId, item: changesSectionItem })),
			}),
			Match.orElse((action) => {
				if (isPanelNavigationAction(action)) {
					handlePanelNavigationAction(action);
					return;
				}

				handleItemSelectionAction(action, selectedItem);
			}),
		);

	const openAbsorptionDialogForItem = (selectedItem: Item) => {
		const worktreeChanges = queryClient.getQueryData(
			changesInWorktreeQueryOptions(projectId).queryKey,
		);
		if (!worktreeChanges) return;

		const target = resolveAbsorptionTarget({
			item: selectedItem,
			worktreeChanges,
		});
		if (!target) return;

		openAbsorptionDialog(target);
	};

	const handleChangesScopeAction = (action: ChangesAction, selectedItem: Item) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => openAbsorptionDialogForItem(selectedItem),
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

	const handleCommitFileScopeAction = (action: CommitFileAction, selectedItem: FileItem) =>
		Match.value(action).pipe(
			Match.tags({
				CloseFiles: () => dispatch(projectActions.closeCommitFiles({ projectId })),
				ToggleFiles: () => {
					if (selectedItem.parent._tag !== "Commit") return;
					dispatch(
						projectActions.toggleCommitFiles({
							projectId,
							item: selectedItem.parent,
						}),
					);
				},
			}),
			Match.orElse((action) => handlePrimaryPanelAction(action, fileItem(selectedItem))),
		);

	const handleBranchScopeAction = (action: BranchAction, selectedItem: BranchItem) =>
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
			Match.orElse((action) => handlePrimaryPanelAction(action, branchItem(selectedItem))),
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
				ChangesFile: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangesScopeAction(action, fileItem(scope.context));
				},
				ChangesSection: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangesScopeAction(action, changesSectionItem);
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
			}),
		);

	const handleShowScopeAction = (action: ShowAction) =>
		Match.value(action).pipe(
			Match.tags({
				CloseShow: () => {
					dispatch(projectActions.hidePanel({ projectId, panel: "show" }));
					focusPanel("primary");
				},
				ToggleShow: () => {
					dispatch(projectActions.togglePanel({ projectId, panel: "show" }));
					focusPanel("primary");
				},
			}),
			Match.orElse((action) => handlePanelNavigationAction(action)),
		);

	const confirmOperationMode = (selectedItem: Item | null) => {
		if (!operationMode) return;

		dispatch(projectActions.exitMode({ projectId }));

		if (!selectedItem) return;

		const operationType = operationModeToOperationType(operationMode);
		const operation = getOperation({
			source: operationMode.source,
			target: selectedItem,
			operationType,
		});
		if (!operation) return;

		runOperation(projectId, operation);
	};

	const handleOperationModeScopeAction = (action: OperationModeAction, selectedItem: Item | null) =>
		Match.value(action).pipe(
			Match.tags({
				Cancel: () => dispatch(projectActions.exitMode({ projectId })),
				Confirm: () => confirmOperationMode(selectedItem),
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
				Cancel: () => {
					dispatch(projectActions.exitMode({ projectId }));
					focusPanel("primary");
				},
				Save: () => inlineRewordCommitFormRef.current?.requestSubmit(),
			}),
		);

	const handleRenameBranchScopeAction = (action: RenameBranchAction) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Cancel: () => {
					dispatch(projectActions.exitMode({ projectId }));
					focusPanel("primary");
				},
				Save: () => inlineRenameBranchFormRef.current?.requestSubmit(),
			}),
		);

	const handleScopeKeyDown = (scope: Scope, event: KeyboardEvent) =>
		isOperationModeScope(scope)
			? handleOperationModeScopeKeyDown(scope, event)
			: Match.value(scope).pipe(
					Match.tagsExhaustive({
						Default: ({ scope }) => handleDefaultScopeKeyDown(scope, event),
						Show: (scope) => {
							const action = getAction(scope.bindings, event);
							if (!action) return;
							event.preventDefault();
							handleShowScopeAction(action);
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
