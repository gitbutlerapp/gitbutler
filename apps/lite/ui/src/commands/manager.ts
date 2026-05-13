import {
	commitDiscardMutationOptions,
	commitInsertBlankMutationOptions,
	unapplyStackMutationOptions,
} from "#ui/api/mutations.ts";
import { absorptionPlanQueryOptions, changesInWorktreeQueryOptions } from "#ui/api/queries.ts";
import {
	getOperation,
	type OperationType,
	useRunOperationMutationOptions,
} from "#ui/operations/operation.ts";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import {
	changesFileParent,
	changesSectionOperand,
	type FileOperand,
	type Operand,
} from "#ui/operands.ts";
import { focusAdjacentPanel, focusPanel, type Panel } from "#ui/panels.ts";
import { isPanelVisible } from "#ui/panels/state.ts";
import {
	projectActions,
	selectProjectDialogState,
	selectProjectOutlineModeState,
	selectProjectPanelsState,
	selectProjectSelectionFiles,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { type CommitAbsorption } from "@gitbutler/but-sdk";
import {
	type Hotkey,
	type HotkeySequence,
	type RegisterableHotkey,
	type UseHotkeyDefinition,
	type UseHotkeyOptions,
	type UseHotkeySequenceDefinition,
	type UseHotkeySequenceOptions,
	normalizeRegisterableHotkey,
	useHotkeySequences,
	useHotkeys,
} from "@tanstack/react-hotkeys";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

export type CommandId =
	| "branch.apply"
	| "branch.rename"
	| "branch.select"
	| "changes.absorb"
	| "changes.commit_mode"
	| "changes.compose_message"
	| "changes.focus_message"
	| "changes.select_changes"
	| "changes_file.absorb"
	| "command_palette.toggle"
	| "commit.add_empty.above"
	| "commit.add_empty.below"
	| "commit.amend"
	| "commit.delete"
	| "commit.reword"
	| "details.toggle"
	| "mode.cancel"
	| "operation.confirm"
	| "operation.move_above"
	| "operation.move_below"
	| "operation.rub"
	| "panel.focus_next"
	| "panel.focus_previous"
	| "selection.cut"
	| "selection.move"
	| "stack.unapply";

export type CommandFn = () => void;

type CommandFns = Partial<Record<CommandId, CommandFn>>;

type CommandHotkey = {
	hotkey: RegisterableHotkey;
} & Omit<UseHotkeyOptions, "target">;

type CommandHotkeySequence = {
	sequence: HotkeySequence;
} & Omit<UseHotkeySequenceOptions, "target">;

type CommandHotkeyDefinition = CommandHotkey | CommandHotkeySequence;

type CommandHotkeys = Partial<Record<CommandId, Array<CommandHotkeyDefinition>>>;

const allCommandHotkeys: CommandHotkeys = {
	"branch.apply": [{ hotkey: "Mod+Shift+A" }],
	"branch.rename": [{ hotkey: "Enter" }],
	"branch.select": [{ hotkey: "T" }],
	"changes.absorb": [{ hotkey: "A" }],
	"changes.commit_mode": [{ hotkey: "C" }],
	"changes.compose_message": [{ hotkey: "Shift+Z" }],
	"changes.focus_message": [{ hotkey: "Enter" }],
	"changes.select_changes": [{ hotkey: "Z" }],
	"changes_file.absorb": [{ hotkey: "A" }],
	"command_palette.toggle": [{ hotkey: "Mod+K" }],
	"commit.amend": [{ hotkey: "Shift+A" }],
	"commit.reword": [{ hotkey: "Enter" }],
	"details.toggle": [{ hotkey: "D" }],
	"mode.cancel": [{ hotkey: "Escape" }],
	"operation.confirm": [{ hotkey: "Mod+V" }, { hotkey: "Enter" }],
	"operation.move_above": [{ hotkey: "A" }],
	"operation.move_below": [{ hotkey: "B" }],
	"operation.rub": [{ hotkey: "R" }],
	"panel.focus_next": [{ hotkey: "L" }],
	"panel.focus_previous": [{ hotkey: "H" }],
	"selection.cut": [{ hotkey: "Mod+X" }, { hotkey: "R" }],
	"selection.move": [{ hotkey: "M" }],
};

export const resolvedCommandHotkeys = (
	hotkeys: Array<CommandHotkeyDefinition> | undefined,
): Array<Hotkey | HotkeySequence> | undefined => {
	const resolved = hotkeys?.flatMap((hotkey) => {
		if (hotkey.enabled === false) return [];
		return "sequence" in hotkey ? [hotkey.sequence] : [normalizeRegisterableHotkey(hotkey.hotkey)];
	});

	return resolved && resolved.length > 0 ? resolved : undefined;
};

const focusedSelection = ({
	focusedPanel,
	selectionFiles,
	selectionOutline,
}: {
	focusedPanel: Panel | null;
	selectionFiles: Operand;
	selectionOutline: Operand;
}) => (focusedPanel === "files" ? selectionFiles : selectionOutline);

const isChangesFile = (operand: Operand): operand is { _tag: "File" } & FileOperand =>
	operand._tag === "File" && operand.parent._tag === changesFileParent._tag;

const focusCommitMessage = () => {
	document.querySelector<HTMLTextAreaElement>('[aria-label="Compose commit message"]')?.focus();
};

const commandHotkeys = (cmds: CommandFns): CommandHotkeys =>
	Object.fromEntries(
		Object.keys(cmds).flatMap((id) => {
			const commandId = id as CommandId;
			const hotkeys = allCommandHotkeys[commandId];
			return cmds[commandId] === undefined || hotkeys === undefined ? [] : [[commandId, hotkeys]];
		}),
	);

export const useProjectCommands = ({
	focusedPanel,
	projectId,
}: {
	focusedPanel: Panel | null;
	projectId: string;
}): [CommandFns, CommandHotkeys] => {
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const changesQuery = useQuery(changesInWorktreeQueryOptions(projectId));
	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const selectionFiles = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	const selectionOutline = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);
	const sourceSelection = focusedSelection({ focusedPanel, selectionFiles, selectionOutline });
	const isDefaultMode = outlineMode._tag === "Default";
	const selectedChangesFile = isChangesFile(selectionFiles) ? selectionFiles : null;
	const selectedChangesFileChange = selectedChangesFile
		? changesQuery.data?.changes.find((change) => change.path === selectedChangesFile.path)
		: undefined;
	const selectedCommit = selectionOutline._tag === "Commit" ? selectionOutline : null;
	const selectedBranch = selectionOutline._tag === "Branch" ? selectionOutline : null;
	const selectedStack = selectionOutline._tag === "Stack" ? selectionOutline : null;
	const keyboardTransfer =
		outlineMode._tag === "Transfer" && outlineMode.value._tag === "Keyboard"
			? outlineMode.value
			: null;
	const keyboardOperation = keyboardTransfer
		? getOperation({
				source: keyboardTransfer.source,
				target: selectionOutline,
				operationType: keyboardTransfer.operationType,
			})
		: null;

	const commitInsertBlank = useMutation({
		...commitInsertBlankMutationOptions,
		onSuccess: async (response, input, context, mutation) => {
			dispatch(
				projectActions.addReplacedCommits({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
				}),
			);

			await commitInsertBlankMutationOptions.onSuccess?.(response, input, context, mutation);
		},
	});
	const commitDiscard = useMutation({
		...commitDiscardMutationOptions,
		onSuccess: async (response, input, context, mutation) => {
			dispatch(
				projectActions.addReplacedCommits({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
				}),
			);

			await commitDiscardMutationOptions.onSuccess?.(response, input, context, mutation);
		},
	});
	const runOperationMutation = useMutation(useRunOperationMutationOptions());
	const unapplyStack = useMutation(unapplyStackMutationOptions);
	const absorbMutation = useMutation({
		mutationFn: (absorptionPlan: Array<CommitAbsorption>) =>
			window.lite.absorb({ projectId, absorptionPlan }),
		onSuccess: async () => {
			await queryClient.invalidateQueries();
		},
	});

	const enterTransferMode = (source: Operand, operationType: OperationType) => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source,
					operationType,
				}),
			}),
		);
		focusPanel("outline");
	};

	const absorbChanges = () => {
		void queryClient
			.fetchQuery(absorptionPlanQueryOptions({ projectId, target: { type: "all" } }))
			.then((absorptionPlan) => {
				dispatch(
					projectActions.enterAbsorbMode({
						projectId,
						source: changesSectionOperand,
						absorptionPlan,
					}),
				);
			});
	};

	const absorbChangesFile = () => {
		if (!selectedChangesFile || !selectedChangesFileChange) return;
		void queryClient
			.fetchQuery(
				absorptionPlanQueryOptions({
					projectId,
					target: {
						type: "treeChanges",
						subject: {
							changes: [selectedChangesFileChange],
							assignedStackId: null,
						},
					},
				}),
			)
			.then((absorptionPlan) => {
				dispatch(
					projectActions.enterAbsorbMode({
						projectId,
						source: selectedChangesFile,
						absorptionPlan,
					}),
				);
				focusPanel("outline");
			});
	};

	const cancelMode = () => {
		dispatch(projectActions.exitMode({ projectId }));
		focusPanel("outline");
	};

	const confirmOperation = () => {
		if (outlineMode._tag === "Absorb") {
			dispatch(projectActions.exitMode({ projectId }));
			if (outlineMode.absorptionPlan.length > 0) absorbMutation.mutate(outlineMode.absorptionPlan);
			return;
		}

		if (!keyboardOperation) return;
		dispatch(projectActions.exitMode({ projectId }));
		runOperationMutation.mutate(keyboardOperation);
	};

	const cmds: CommandFns = {
		"branch.apply": () => dispatch(projectActions.openApplyBranchPicker({ projectId })),
		"branch.select": () => dispatch(projectActions.openBranchPicker({ projectId })),
		"changes.compose_message": () => {
			dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
			focusCommitMessage();
		},
		"changes.select_changes": () => {
			dispatch(projectActions.selectOutline({ projectId, selection: changesSectionOperand }));
			focusPanel("outline");
		},
		"command_palette.toggle": () => {
			if (dialog._tag === "CommandPalette") dispatch(projectActions.closeDialog({ projectId }));
			else dispatch(projectActions.openCommandPalette({ projectId, focusedPanel }));
		},
		"details.toggle": () => {
			if (focusedPanel === "details" && isPanelVisible(panelsState, "details")) {
				const detailsPanelIndex = panelsState.visiblePanels.indexOf("details");
				const nextPanel = panelsState.visiblePanels[detailsPanelIndex - 1];
				if (nextPanel !== undefined) focusPanel(nextPanel);
			}

			dispatch(projectActions.togglePanel({ projectId, panel: "details" }));
		},
	};

	if (selectedBranch && focusedPanel === "outline" && isDefaultMode)
		cmds["branch.rename"] = () =>
			dispatch(projectActions.startRenameBranch({ projectId, branch: selectedBranch }));

	if (
		changesQuery.data &&
		changesQuery.data.changes.length > 0 &&
		selectionOutline._tag === "ChangesSection" &&
		focusedPanel === "outline" &&
		isDefaultMode
	)
		cmds["changes.absorb"] = absorbChanges;

	if (focusedPanel !== null && isDefaultMode) {
		cmds["changes.commit_mode"] = () => enterTransferMode(changesSectionOperand, "moveAbove");
		cmds["selection.cut"] = () => enterTransferMode(sourceSelection, "rub");
		cmds["selection.move"] = () => enterTransferMode(sourceSelection, "moveAbove");
	}

	if (selectionOutline._tag === "ChangesSection" && focusedPanel === "outline" && isDefaultMode)
		cmds["changes.focus_message"] = focusCommitMessage;

	if (selectedChangesFile && selectedChangesFileChange && focusedPanel === "files" && isDefaultMode)
		cmds["changes_file.absorb"] = absorbChangesFile;

	if (selectedCommit && focusedPanel === "outline" && isDefaultMode) {
		cmds["commit.add_empty.above"] = () =>
			commitInsertBlank.mutate({
				projectId,
				relativeTo: { type: "commit", subject: selectedCommit.commitId },
				side: "above",
				dryRun: false,
			});
		cmds["commit.add_empty.below"] = () =>
			commitInsertBlank.mutate({
				projectId,
				relativeTo: { type: "commit", subject: selectedCommit.commitId },
				side: "below",
				dryRun: false,
			});
		cmds["commit.amend"] = () => enterTransferMode(changesSectionOperand, "rub");
		cmds["commit.reword"] = () =>
			dispatch(projectActions.startRewordCommit({ projectId, commit: selectedCommit }));

		if (!commitDiscard.isPending)
			cmds["commit.delete"] = () =>
				commitDiscard.mutate({
					projectId,
					subjectCommitId: selectedCommit.commitId,
					dryRun: false,
				});
	}

	if (outlineMode._tag !== "Default") cmds["mode.cancel"] = cancelMode;

	if ((outlineMode._tag === "Absorb" && outlineMode.absorptionPlan.length > 0) || keyboardOperation)
		cmds["operation.confirm"] = confirmOperation;

	if (keyboardTransfer) {
		cmds["operation.move_above"] = () =>
			dispatch(
				projectActions.updateTransferOperationType({ projectId, operationType: "moveAbove" }),
			);
		cmds["operation.move_below"] = () =>
			dispatch(
				projectActions.updateTransferOperationType({ projectId, operationType: "moveBelow" }),
			);
		cmds["operation.rub"] = () =>
			dispatch(projectActions.updateTransferOperationType({ projectId, operationType: "rub" }));
	}

	if (focusedPanel !== null) {
		cmds["panel.focus_next"] = () => focusAdjacentPanel(1);
		cmds["panel.focus_previous"] = () => focusAdjacentPanel(-1);
	}

	if (selectedStack && focusedPanel === "outline" && isDefaultMode && !unapplyStack.isPending)
		cmds["stack.unapply"] = () =>
			unapplyStack.mutate({ projectId, stackId: selectedStack.stackId });

	return [cmds, commandHotkeys(cmds)];
};

export const useCommandHotkeys = (cmds: CommandFns, hotkeys: CommandHotkeys) => {
	const { hotkeyDefs, sequenceDefs } = Object.entries(hotkeys).reduce(
		(acc, [id, hotkeys]) => {
			for (const hotkey of hotkeys) {
				const enabled = hotkey.enabled !== false;
				const def: UseHotkeyDefinition | UseHotkeySequenceDefinition = {
					callback: () => cmds[id as CommandId]?.(),
					options: {
						enabled,
						conflictBehavior: enabled ? "warn" : "allow",
						...hotkey,
					},
					...hotkey,
				};

				if ("sequence" in hotkey) acc.sequenceDefs.push(def as UseHotkeySequenceDefinition);
				else acc.hotkeyDefs.push(def as UseHotkeyDefinition);
			}

			return acc;
		},
		{
			hotkeyDefs: [] as Array<UseHotkeyDefinition>,
			sequenceDefs: [] as Array<UseHotkeySequenceDefinition>,
		},
	);

	useHotkeys(hotkeyDefs);
	useHotkeySequences(sequenceDefs);
};
