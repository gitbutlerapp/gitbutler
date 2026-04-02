import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
} from "#ui/api/queries.ts";
import { useFullscreenPreview } from "#ui/hooks/useFullscreenPreview.ts";
import { usePreviewPanel } from "#ui/hooks/usePreviewPanel.ts";
import { getAction, type ShortcutBinding } from "#ui/shortcuts.ts";
import { isTypingTarget } from "#ui/routes/project/$id/-shared.tsx";
import { TreeChange } from "@gitbutler/but-sdk";
import { useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { useEffect, useEffectEvent } from "react";
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
	buildNavigationModel,
	getAdjacentCommitDetailsPath,
	getAdjacentItem,
	getAdjacentSection,
	getSelectedCommitPath,
} from "./-Selection.ts";

type SelectionAction =
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "PreviousSection" }
	| { _tag: "NextSection" }
	| { _tag: "TogglePreview" }
	| { _tag: "OpenFullscreenPreview" };

type ChangesAction = SelectionAction | { _tag: "Absorb" };

type CommitSummaryAction = SelectionAction | { _tag: "EditMessage" } | { _tag: "OpenDetails" };

type CommitDetailsAction = SelectionAction | { _tag: "CloseDetails" };

export const togglePreviewBinding: ShortcutBinding<SelectionAction> = {
	id: "toggle-preview",
	description: "Preview",
	keys: ["p"],
	action: { _tag: "TogglePreview" },
	repeat: false,
};

export const openFullscreenPreviewBinding: ShortcutBinding<SelectionAction> = {
	id: "open-fullscreen-preview",
	description: "Open fullscreen preview",
	keys: ["d"],
	action: { _tag: "OpenFullscreenPreview" },
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
	},
	{
		id: "next-section",
		description: "Next section",
		keys: ["Shift+ArrowDown", "Shift+j"],
		action: { _tag: "NextSection" },
	},
	togglePreviewBinding,
	openFullscreenPreviewBinding,
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
	description: "Edit message",
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

type BranchSegmentAction = SelectionAction | { _tag: "RenameBranch" } | { _tag: "RemoveBranch" };

const branchSegmentBindings: Array<ShortcutBinding<BranchSegmentAction>> = [
	...selectionBindings,
	{
		id: "branch-segment-rename",
		description: "Rename",
		keys: ["Enter"],
		action: { _tag: "RenameBranch" },
		repeat: false,
	},
	{
		id: "branch-segment-remove",
		description: "Remove branch",
		keys: ["Backspace"],
		action: { _tag: "RemoveBranch" },
		repeat: false,
	},
];

type FullscreenPreviewAction = { _tag: "Close" };

export const closeFullscreenPreviewBinding: ShortcutBinding<FullscreenPreviewAction> = {
	id: "close-fullscreen-preview",
	description: "Close",
	keys: ["Escape"],
	action: { _tag: "Close" },
	repeat: false,
};

const fullscreenPreviewBindings: Array<ShortcutBinding<FullscreenPreviewAction>> = [
	closeFullscreenPreviewBinding,
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
			_tag: "FullscreenPreview";
			bindings: Array<ShortcutBinding<FullscreenPreviewAction>>;
	  }
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
			_tag: "CommitEditMessage";
			bindings: Array<ShortcutBinding<CommitEditingMessageAction>>;
			context: CommitItem;
	  }
	| {
			_tag: "RenameBranch";
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
			bindings: Array<ShortcutBinding<BranchSegmentAction>>;
			context: SegmentItem;
	  };

export const getScope = ({
	showFullscreenPreview,
	selection,
	editing,
}: {
	showFullscreenPreview: boolean;
	selection: Item | null;
	editing: Editing | null;
}): Scope | null => {
	if (showFullscreenPreview)
		return {
			_tag: "FullscreenPreview",
			bindings: fullscreenPreviewBindings,
		};

	if (!selection) return null;

	return Match.value(selection).pipe(
		Match.tag(
			"Changes",
			(selection): Scope => ({
				_tag: "Changes",
				bindings: changesBindings,
				context: selection,
			}),
		),
		Match.tag("Commit", (selection): Scope => {
			if (
				editing?._tag === "CommitMessage" &&
				editing.subject.stackId === selection.stackId &&
				editing.subject.segmentIndex === selection.segmentIndex &&
				editing.subject.commitId === selection.commitId
			)
				return {
					_tag: "CommitEditMessage",
					bindings: commitEditingMessageBindings,
					context: selection,
				};

			return Match.value(selection.mode).pipe(
				Match.tagsExhaustive({
					Details: (): Scope => ({
						_tag: "CommitDetails",
						bindings: commitDetailsBindings,
						context: selection,
					}),
					Summary: (): Scope => ({
						_tag: "CommitSummary",
						bindings: commitSummaryBindings,
						context: selection,
					}),
				}),
			);
		}),
		Match.tag(
			"BaseCommit",
			(selection): Scope => ({
				_tag: "BaseCommit",
				bindings: selectionBindings,
				context: selection,
			}),
		),
		Match.tag(
			"Segment",
			(selection): Scope =>
				editing?._tag === "BranchName" &&
				editing.subject.stackId === selection.stackId &&
				editing.subject.segmentIndex === selection.segmentIndex
					? {
							_tag: "RenameBranch",
							bindings: renameBranchBindings,
							context: selection,
						}
					: selection.branchName === null
						? {
								_tag: "Segment",
								bindings: selectionBindings,
								context: selection,
							}
						: {
								_tag: "Branch",
								bindings: branchSegmentBindings,
								context: selection,
							},
		),
		Match.exhaustive,
	);
};

export const getLabel = (scope: Scope): string =>
	Match.value(scope).pipe(
		Match.tagsExhaustive({
			FullscreenPreview: () => "Fullscreen preview",
			BaseCommit: () => "Base commit",
			RenameBranch: () => "Rename branch",
			Changes: () => "Changes",
			CommitDetails: () => "Commit details",
			CommitEditMessage: () => "Edit commit message",
			CommitSummary: () => "Commit",
			Branch: () => "Branch",
			Segment: () => "Segment",
		}),
	);

export const useWorkspaceShortcuts = ({
	projectId,
	scope,
	select,
	setEditing,
	commonBaseCommitId,
	onAbsorbChanges,
	onRemoveBranch,
}: {
	projectId: string;
	scope: Scope | null;
	select: (selection: Item | null) => void;
	setEditing: (selection: Editing | null) => void;
	commonBaseCommitId?: string;
	onAbsorbChanges: (changes: Array<TreeChange>, stackId: string | null) => void;
	onRemoveBranch: (selection: SegmentItem) => void;
}) => {
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const [, setShowPreviewPanel] = usePreviewPanel();
	const [, setShowFullscreenPreview] = useFullscreenPreview(projectId);

	const queryClient = useQueryClient();
	const navigationModel = buildNavigationModel({
		headInfo,
		changes: worktreeChanges.changes,
		assignments: worktreeChanges.assignments,
		commonBaseCommitId,
	});

	const moveCommitDetails = (offset: -1 | 1, selection: CommitItem) => {
		const commitDetails = queryClient.getQueryData(
			commitDetailsWithLineStatsQueryOptions({
				projectId,
				commitId: selection.commitId,
			}).queryKey,
		);
		if (!commitDetails) return;

		const paths = commitDetails.changes.map((change) => change.path);
		const nextPath = getAdjacentCommitDetailsPath({
			paths,
			currentPath: getSelectedCommitPath({ paths, selection }),
			offset,
		});
		if (nextPath === null) return;

		select(
			commitItem({
				...selection,
				mode: { _tag: "Details", path: nextPath },
			}),
		);
	};

	const move = (offset: -1 | 1, selection: Item) =>
		select(getAdjacentItem(navigationModel, selection, offset));
	const previousSection = (selection: Item) =>
		select(getParentSection(selection) ?? getAdjacentSection(navigationModel, selection, -1));
	const nextSection = (selection: Item) =>
		select(getAdjacentSection(navigationModel, selection, 1));

	const handleSelectionAction = (action: SelectionAction, selection: Item) =>
		Match.value(action).pipe(
			Match.tagsExhaustive({
				Move: ({ offset }) => move(offset, selection),
				PreviousSection: () => previousSection(selection),
				NextSection: () => nextSection(selection),
				TogglePreview: () => setShowPreviewPanel((visible) => !visible),
				OpenFullscreenPreview: () => setShowFullscreenPreview(true),
			}),
		);

	const handleChangesAction = (action: ChangesAction, selection: ChangesItem) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => {
					Match.value(selection.mode).pipe(
						Match.tagsExhaustive({
							Details: ({ path }) => {
								if (path === undefined) return;
								const change = worktreeChanges.changes.find((change) => change.path === path);
								if (!change) return;
								onAbsorbChanges([change], selection.stackId);
							},
							Summary: () => {
								const assignmentsByPath = new Set(
									worktreeChanges.assignments
										.filter((assignment) => assignment.stackId === selection.stackId)
										.map((assignment) => assignment.path),
								);
								const changes = worktreeChanges.changes.filter((change) =>
									assignmentsByPath.has(change.path),
								);
								onAbsorbChanges(changes, selection.stackId);
							},
						}),
					);
				},
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Changes", ...selection })),
		);

	const handleCommitSummaryAction = (action: CommitSummaryAction, selection: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				EditMessage: () => setEditing({ _tag: "CommitMessage", subject: selection }),
				OpenDetails: () => select(commitItem({ ...selection, mode: { _tag: "Details" } })),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Commit", ...selection })),
		);

	const handleCommitDetailsAction = (action: CommitDetailsAction, selection: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				Move: ({ offset }) => moveCommitDetails(offset, selection),
				CloseDetails: () => select(commitItem({ ...selection, mode: { _tag: "Summary" } })),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Commit", ...selection })),
		);

	const handleBranchSegmentAction = (action: BranchSegmentAction, selection: SegmentItem) =>
		Match.value(action).pipe(
			Match.tags({
				RenameBranch: () => {
					setEditing({
						_tag: "BranchName",
						subject: {
							stackId: selection.stackId,
							segmentIndex: selection.segmentIndex,
						},
					});
				},
				RemoveBranch: () => onRemoveBranch(selection),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Segment", ...selection })),
		);

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented) return;
		if (isTypingTarget(event.target)) return;

		if (!scope) return;

		Match.value(scope).pipe(
			Match.tagsExhaustive({
				FullscreenPreview: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					Match.value(action).pipe(
						Match.tagsExhaustive({
							Close: () => setShowFullscreenPreview(false),
						}),
					);
				},
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
					handleBranchSegmentAction(action, scope.context);
				},
				RenameBranch: () => undefined,
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
				CommitEditMessage: () => undefined,
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
