import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { useFullscreenPreview } from "#ui/hooks/useFullscreenPreview.ts";
import { usePreviewPanel } from "#ui/hooks/usePreviewPanel.ts";
import { getAction, type ShortcutBinding } from "#ui/shortcuts.ts";
import { assignedHunks, isTypingTarget } from "#ui/routes/project/$id/-shared.tsx";
import { AbsorptionTarget } from "@gitbutler/but-sdk";
import { useQueryClient } from "@tanstack/react-query";
import { Match } from "effect";
import { useEffect, useEffectEvent } from "react";
import { type Editing } from "./-Editing.ts";
import {
	closeChangeFileDetails,
	closeCommitFileDetails,
	openChangeFileDetails,
	openCommitFileDetails,
} from "./-FileDetails.ts";
import {
	commitItem,
	changesDetailsItem,
	detailsFileItem,
	detailsHunkItem,
	type Item,
	CommitItem,
	getParentSection,
	SegmentItem,
	ChangesItem,
	BaseCommitItem,
} from "./-Item.ts";
import {
	getAdjacentHunk,
	getAdjacentPath,
	getAdjacentItem,
	getAdjacentSection,
	type NavigationModel,
} from "./-Selection.ts";

type AbsorbAction = { _tag: "Absorb" };

export const absorbBinding: ShortcutBinding<AbsorbAction> = {
	id: "absorb",
	description: "Absorb",
	keys: ["a"],
	action: { _tag: "Absorb" },
	repeat: false,
};

type SelectionAction =
	| { _tag: "Move"; offset: -1 | 1 }
	| { _tag: "PreviousSection" }
	| { _tag: "NextSection" }
	| { _tag: "TogglePreview" }
	| { _tag: "OpenFullscreenPreview" };

type ChangesSummaryAction = SelectionAction | AbsorbAction;

type ChangeDetailsAction = SelectionAction | AbsorbAction | { _tag: "OpenFileDetails" };

type ChangeFileDetailsAction = SelectionAction | { _tag: "CloseDetails" } | AbsorbAction;

type CommitSummaryAction = SelectionAction | { _tag: "EditMessage" } | { _tag: "OpenDetails" };

type CommitDetailsAction = SelectionAction | { _tag: "CloseDetails" } | { _tag: "OpenFileDetails" };

type CommitFileDetailsAction = SelectionAction | { _tag: "CloseDetails" };

export const togglePreviewBinding: ShortcutBinding<SelectionAction> = {
	id: "toggle-preview",
	description: "Preview",
	keys: ["p"],
	action: { _tag: "TogglePreview" },
	repeat: false,
};

export const openFullscreenPreviewBinding: ShortcutBinding<SelectionAction> = {
	id: "open-fullscreen-preview",
	description: "Fullscreen preview",
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
		showInShortcutsBar: false,
	},
	{
		id: "next-section",
		description: "Next section",
		keys: ["Shift+ArrowDown", "Shift+j"],
		action: { _tag: "NextSection" },
		showInShortcutsBar: false,
	},
	togglePreviewBinding,
	openFullscreenPreviewBinding,
];

const changesSummaryBindings: Array<ShortcutBinding<ChangesSummaryAction>> = [
	...selectionBindings,
	absorbBinding,
];

export const openChangeFileDetailsBinding: ShortcutBinding<ChangeDetailsAction> = {
	id: "changes-open-file-details",
	description: "Open details",
	keys: ["ArrowRight", "l"],
	action: { _tag: "OpenFileDetails" },
	repeat: false,
};

const changeDetailsBindings: Array<ShortcutBinding<ChangeDetailsAction>> = [
	...selectionBindings,
	openChangeFileDetailsBinding,
	absorbBinding,
];

export const closeChangeFileDetailsBinding: ShortcutBinding<ChangeFileDetailsAction> = {
	id: "change-file-details-close",
	description: "Close details",
	keys: ["ArrowLeft", "Escape"],
	action: { _tag: "CloseDetails" },
	repeat: false,
};

const changesFileDetailsBindings: Array<ShortcutBinding<ChangeFileDetailsAction>> = [
	...selectionBindings,
	absorbBinding,
	closeChangeFileDetailsBinding,
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

export const openCommitFileDetailsBinding: ShortcutBinding<CommitDetailsAction> = {
	id: "commit-open-file-details",
	description: "Open details",
	keys: ["ArrowRight", "l"],
	action: { _tag: "OpenFileDetails" },
	repeat: false,
};

const commitDetailsBindings: Array<ShortcutBinding<CommitDetailsAction>> = [
	...selectionBindings,
	closeCommitDetailsBinding,
	openCommitFileDetailsBinding,
];

export const closeCommitFileDetailsBinding: ShortcutBinding<CommitFileDetailsAction> = {
	id: "commit-file-details-close",
	description: "Close details",
	keys: ["ArrowLeft", "Escape"],
	action: { _tag: "CloseDetails" },
	repeat: false,
};

const commitFileDetailsBindings: Array<ShortcutBinding<CommitFileDetailsAction>> = [
	...selectionBindings,
	closeCommitFileDetailsBinding,
];

type BranchSegmentAction = SelectionAction | { _tag: "RenameBranch" };

const branchSegmentBindings: Array<ShortcutBinding<BranchSegmentAction>> = [
	...selectionBindings,
	{
		id: "segment-rename-branch",
		description: "Rename",
		keys: ["Enter"],
		action: { _tag: "RenameBranch" },
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
			_tag: "ChangesSummary";
			bindings: Array<ShortcutBinding<ChangesSummaryAction>>;
			context: ChangesItem;
	  }
	| {
			_tag: "ChangeDetails";
			bindings: Array<ShortcutBinding<ChangeDetailsAction>>;
			context: ChangesItem;
	  }
	| {
			_tag: "CommitDetails";
			bindings: Array<ShortcutBinding<CommitDetailsAction>>;
			context: CommitItem;
	  }
	| {
			_tag: "ChangeFileDetails";
			bindings: Array<ShortcutBinding<ChangeFileDetailsAction>>;
			context: ChangesItem;
	  }
	| {
			_tag: "CommitFileDetails";
			bindings: Array<ShortcutBinding<CommitFileDetailsAction>>;
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
			(selection): Scope =>
				Match.value(selection.mode).pipe(
					Match.tagsExhaustive({
						Summary: (): Scope => ({
							_tag: "ChangesSummary",
							bindings: changesSummaryBindings,
							context: selection,
						}),
						Details: ({ item }): Scope =>
							Match.value(item).pipe(
								Match.tagsExhaustive({
									Hunk: (): Scope => ({
										_tag: "ChangeFileDetails",
										bindings: changesFileDetailsBindings,
										context: selection,
									}),
									File: (): Scope => ({
										_tag: "ChangeDetails",
										bindings: changeDetailsBindings,
										context: selection,
									}),
								}),
							),
					}),
				),
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
					Details: ({ item }): Scope =>
						item?._tag === "Hunk"
							? {
									_tag: "CommitFileDetails",
									bindings: commitFileDetailsBindings,
									context: selection,
								}
							: {
									_tag: "CommitDetails",
									bindings: commitDetailsBindings,
									context: selection,
								},
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
			ChangesSummary: () => "Changes",
			ChangeDetails: () => "Change details",
			CommitDetails: () => "Commit details",
			ChangeFileDetails: () => "Change file details",
			CommitFileDetails: () => "Commit file details",
			CommitEditMessage: () => "Reword commit",
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
	navigationModel,
	requestAbsorptionPlan,
}: {
	projectId: string;
	scope: Scope | null;
	select: (selection: Item | null) => void;
	setEditing: (selection: Editing | null) => void;
	navigationModel: NavigationModel;
	requestAbsorptionPlan: (target: AbsorptionTarget) => void;
}) => {
	const [, setShowPreviewPanel] = usePreviewPanel();
	const [, setShowFullscreenPreview] = useFullscreenPreview(projectId);

	const queryClient = useQueryClient();

	const requestAbsorptionPlanForSelection = (selection: ChangesItem) => {
		const worktreeChanges = queryClient.getQueryData(
			changesInWorktreeQueryOptions(projectId).queryKey,
		);
		if (!worktreeChanges) return;

		Match.value(selection.mode).pipe(
			Match.tagsExhaustive({
				Details: ({ item }) => {
					const change = worktreeChanges.changes.find((change) => change.path === item.path);
					if (!change) return;

					Match.value(item).pipe(
						Match.tagsExhaustive({
							File: () =>
								requestAbsorptionPlan({
									type: "treeChanges",
									subject: {
										changes: [change],
										assigned_stack_id: selection.stackId,
									},
								}),
							Hunk: ({ hunkHeader }) =>
								requestAbsorptionPlan({
									type: "hunkAssignments",
									subject: {
										assignments: [
											{
												path: change.path,
												pathBytes: change.pathBytes,
												hunkHeader,
												stackId: selection.stackId,
											},
										],
									},
								}),
						}),
					);
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
					requestAbsorptionPlan({
						type: "treeChanges",
						subject: {
							changes,
							assigned_stack_id: selection.stackId,
						},
					});
				},
			}),
		);
	};

	const moveCommitDetailsFile = ({
		offset,
		selection,
	}: {
		offset: -1 | 1;
		selection: CommitItem;
	}) => {
		if (selection.mode._tag !== "Details") return;

		const commitDetails = queryClient.getQueryData(
			commitDetailsWithLineStatsQueryOptions({
				projectId,
				commitId: selection.commitId,
			}).queryKey,
		);
		if (!commitDetails) return;

		const paths = commitDetails.changes.map((change) => change.path);
		const currentPath = selection.mode.item?.path;
		if (currentPath === undefined) return;

		const nextPath = getAdjacentPath({ paths, currentPath, offset });
		if (nextPath === null) return;

		select(
			commitItem({
				...selection,
				mode: { _tag: "Details", item: detailsFileItem(nextPath) },
			}),
		);
	};

	const openCommitDetails = async (selection: CommitItem) => {
		const commitDetails = await queryClient
			.fetchQuery(
				commitDetailsWithLineStatsQueryOptions({
					projectId,
					commitId: selection.commitId,
				}),
			)
			.catch(() => null);
		if (!commitDetails) return;

		const firstPath = commitDetails.changes[0]?.path;

		select(
			commitItem({
				...selection,
				mode:
					firstPath === undefined
						? { _tag: "Details", item: null }
						: { _tag: "Details", item: detailsFileItem(firstPath) },
			}),
		);
	};

	const moveCommitDetailsHunk = ({
		offset,
		selection,
	}: {
		offset: -1 | 1;
		selection: CommitItem;
	}) => {
		if (selection.mode._tag !== "Details" || selection.mode.item?._tag !== "Hunk") return;

		const currentPath = selection.mode.item.path;

		const commitDetails = queryClient.getQueryData(
			commitDetailsWithLineStatsQueryOptions({
				projectId,
				commitId: selection.commitId,
			}).queryKey,
		);
		if (!commitDetails) return;

		const change = commitDetails.changes.find((change) => change.path === currentPath);
		if (!change) return;

		const diff = queryClient.getQueryData(
			treeChangeDiffsQueryOptions({ projectId, change }).queryKey,
		);
		if (!diff || diff.type !== "Patch") return;

		const nextHunk = getAdjacentHunk({
			hunks: diff.subject.hunks,
			currentHunk: selection.mode.item.hunkHeader,
			offset,
		});
		if (nextHunk === null) return;

		select(
			commitItem({
				...selection,
				mode: { _tag: "Details", item: detailsHunkItem(currentPath, nextHunk) },
			}),
		);
	};

	const moveChangesDetailsHunk = (offset: -1 | 1, selection: ChangesItem) => {
		if (selection.mode._tag !== "Details" || selection.mode.item._tag !== "Hunk") return;
		const currentItem = selection.mode.item;

		const worktreeChanges = queryClient.getQueryData(
			changesInWorktreeQueryOptions(projectId).queryKey,
		);
		if (!worktreeChanges) return;

		const change = worktreeChanges.changes.find((change) => change.path === currentItem.path);
		if (!change) return;

		const diff = queryClient.getQueryData(
			treeChangeDiffsQueryOptions({ projectId, change }).queryKey,
		);
		if (!diff || diff.type !== "Patch") return;

		const assignments = worktreeChanges.assignments.filter(
			(assignment) =>
				(assignment.stackId ?? null) === selection.stackId && assignment.path === change.path,
		);
		const nextHunk = getAdjacentHunk({
			hunks: assignedHunks(diff.subject.hunks, assignments),
			currentHunk: currentItem.hunkHeader,
			offset,
		});
		if (nextHunk === null) return;

		select(changesDetailsItem(selection.stackId, detailsHunkItem(change.path, nextHunk)));
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

	const handleChangesAction = (action: ChangesSummaryAction, selection: ChangesItem) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => requestAbsorptionPlanForSelection(selection),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Changes", ...selection })),
		);

	const handleChangeDetailsAction = (action: ChangeDetailsAction, selection: ChangesItem) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => requestAbsorptionPlanForSelection(selection),
				OpenFileDetails: () => {
					void openChangeFileDetails({ projectId, queryClient, select, selection });
				},
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Changes", ...selection })),
		);

	const handleChangeFileDetailsAction = (action: ChangeFileDetailsAction, selection: ChangesItem) =>
		Match.value(action).pipe(
			Match.tags({
				Absorb: () => requestAbsorptionPlanForSelection(selection),
				Move: ({ offset }) => moveChangesDetailsHunk(offset, selection),
				CloseDetails: () => closeChangeFileDetails({ select, selection }),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Changes", ...selection })),
		);

	const handleCommitSummaryAction = (action: CommitSummaryAction, selection: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				EditMessage: () => setEditing({ _tag: "CommitMessage", subject: selection }),
				OpenDetails: () => {
					void openCommitDetails(selection);
				},
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Commit", ...selection })),
		);

	const handleCommitDetailsAction = (action: CommitDetailsAction, selection: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				Move: ({ offset }) => moveCommitDetailsFile({ offset, selection }),
				OpenFileDetails: () => {
					void openCommitFileDetails({ projectId, queryClient, select, selection });
				},
				CloseDetails: () => select(commitItem({ ...selection, mode: { _tag: "Summary" } })),
			}),
			Match.orElse((action) => handleSelectionAction(action, { _tag: "Commit", ...selection })),
		);

	const handleCommitFileDetailsAction = (action: CommitFileDetailsAction, selection: CommitItem) =>
		Match.value(action).pipe(
			Match.tags({
				Move: ({ offset }) => moveCommitDetailsHunk({ offset, selection }),
				CloseDetails: () => closeCommitFileDetails({ select, selection }),
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
				ChangesSummary: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangesAction(action, scope.context);
				},
				ChangeDetails: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangeDetailsAction(action, scope.context);
				},
				ChangeFileDetails: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleChangeFileDetailsAction(action, scope.context);
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
				CommitFileDetails: (scope) => {
					const action = getAction(scope.bindings, event);
					if (!action) return;
					event.preventDefault();
					handleCommitFileDetailsAction(action, scope.context);
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
