import uiStyles from "#ui/ui/ui.module.css";
import {
	commitCreateMutationOptions,
	commitDiscardMutationOptions,
	commitInsertBlankMutationOptions,
	commitRewordMutationOptions,
	unapplyStackMutationOptions,
	updateBranchNameMutationOptions,
} from "#ui/api/mutations.ts";
import {
	absorptionPlanQueryOptions,
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
} from "#ui/api/queries.ts";
import { findCommit, getCommonBaseCommitId } from "#ui/api/ref-info.ts";
import { encodeRefName } from "#ui/api/ref-name.ts";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import {
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import {
	baseCommitOperand,
	branchOperand,
	changesSectionOperand,
	commitOperand,
	operandEquals,
	operandIdentityKey,
	stackOperand,
	type BranchOperand,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import { filterNavigationIndexForOutlineMode, getBinaryOperation } from "#ui/outline/mode.ts";
import { focusPanel, useFocusedProjectPanel, useNavigationIndexHotkeys } from "#ui/panels.ts";
import {
	projectActions,
	selectProjectHighlightedCommitIds,
	selectProjectOperationModeState,
	selectProjectOutlineModeState,
	selectProjectReplacedCommits,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { OperationSourceLabel } from "#ui/routes/project/$id/workspace/OperationSourceLabel.tsx";
import { OperationTarget } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/ui/classes.ts";
import { MenuTriggerIcon, PushIcon } from "#ui/ui/icons.tsx";
import {
	buildNavigationIndex,
	navigationIndexIncludes,
	Section,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { mergeProps, Toast, useRender } from "@base-ui/react";
import { Combobox } from "@base-ui/react/combobox";
import { Toolbar } from "@base-ui/react/toolbar";
import {
	AbsorptionTarget,
	BranchReference,
	Commit,
	RefInfo,
	Segment,
	Stack,
	TreeChange,
	WorkspaceState,
} from "@gitbutler/but-sdk";
import { formatForDisplay } from "@tanstack/react-hotkeys";
import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";

import {
	ComponentProps,
	createContext,
	FC,
	Fragment,
	Suspense,
	use,
	useEffect,
	useOptimistic,
	useRef,
	useState,
	useTransition,
} from "react";
import { Panel, PanelProps } from "react-resizable-panels";
import styles from "./OutlinePanel.module.css";
import workspaceItemRowStyles from "./WorkspaceItemRow.module.css";
import { WorkspaceItemRow, WorkspaceItemRowToolbar } from "./WorkspaceItemRow.tsx";
import {
	moveOperation,
	useDryRunOperation,
	useRunOperationMutationOptions,
} from "#ui/operations/operation.ts";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";
import { defaultOutlineSelection } from "#ui/projects/workspace/state.ts";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { useResolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import { rejectedChangesToastOptions } from "#ui/operations/rejectedChangesToastOptions.tsx";
import { useCommand } from "#ui/commands/manager.ts";
import { assert } from "#ui/assert.ts";

const NavigationIndexContext = createContext<NavigationIndex | null>(null);

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);

const useDryRunCommit = (commitId: string) => {
	const dryRunWorkspace = use(DryRunWorkspaceContext);
	if (!dryRunWorkspace) return null;

	const dryRunCommitId = dryRunWorkspace.replacedCommits[commitId] ?? commitId;
	return findCommit({ headInfo: dryRunWorkspace.headInfo, commitId: dryRunCommitId });
};

const sections = (headInfo: RefInfo): NonEmptyArray<Section> => {
	const changesSection: Section = {
		section: changesSectionOperand,
		children: [],
	};

	const segmentChildren = (stackId: string, segment: Segment): Array<Operand> =>
		segment.commits.map((commit) => commitOperand({ stackId, commitId: commit.id }));

	const segmentSection = (stackId: string, segment: Segment): Section | null => {
		const children = segmentChildren(stackId, segment);
		const branchRef = segment.refName?.fullNameBytes;
		if (!branchRef && children.length === 0) return null;

		return {
			section: branchRef ? branchOperand({ stackId, branchRef }) : null,
			children,
		};
	};

	const baseCommitSection: Section = {
		section: baseCommitOperand,
		children: [],
	};

	return [
		changesSection,

		...headInfo.stacks.flatMap((stack) => {
			// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
			const stackId = stack.id!;
			const stackOperandSection: Section = {
				section: stackOperand({ stackId }),
				children: [],
			};
			return [
				stackOperandSection,
				...stack.segments.flatMap((segment) => {
					const section = segmentSection(stackId, segment);
					return section ? [section] : [];
				}),
			];
		}),

		baseCommitSection,
	];
};

const useNavigationIndex = (projectId: string) => {
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const navigationIndexUnfiltered = buildNavigationIndex(sections(headInfo));

	const selection = useAppSelector((state) => selectProjectSelectionOutline(state, projectId));
	const replacedCommits = useAppSelector((state) => selectProjectReplacedCommits(state, projectId));

	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		//
		// Update selection when the commit was replaced.
		//
		const updatedSelection = Match.value(selection).pipe(
			Match.withReturnType<Operand | null>(),
			Match.tags({
				Commit: (selection) => {
					const newCommitId = replacedCommits[selection.commitId];
					if (newCommitId === undefined || newCommitId === selection.commitId) return null;

					return commitOperand({ ...selection, commitId: newCommitId });
				},
			}),
			Match.orElse(() => null),
		);

		if (updatedSelection && navigationIndexIncludes(navigationIndexUnfiltered, updatedSelection)) {
			dispatch(
				projectActions.selectOutline({
					projectId,
					selection: updatedSelection,
				}),
			);
			return;
		}

		//
		// Reset selection when it's no longer part of the workspace.
		//
		if (!navigationIndexIncludes(navigationIndexUnfiltered, selection))
			dispatch(
				projectActions.selectOutline({
					projectId,
					selection: defaultOutlineSelection,
				}),
			);
	}, [navigationIndexUnfiltered, selection, projectId, dispatch, replacedCommits]);

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const navigationIndex = filterNavigationIndexForOutlineMode({
		navigationIndex: navigationIndexUnfiltered,
		outlineMode,
	});

	const focusedPanel = useFocusedProjectPanel(projectId);

	const select = (newItem: Operand) =>
		dispatch(projectActions.selectOutline({ projectId, selection: newItem }));

	useNavigationIndexHotkeys({
		focusedPanel,
		navigationIndex,
		projectId,
		group: "Outline",
		panel: "outline",
		select,
		selection,
	});

	return navigationIndex;
};

export const OutlinePanel: FC<PanelProps> = ({ ...panelProps }) => (
	<Suspense
		fallback={
			<Panel {...panelProps} className={classes(panelProps.className, styles.panelPadding)}>
				Loading outline…
			</Panel>
		}
	>
		<OutlineTreePanel {...panelProps} />
	</Suspense>
);

const OutlineTreePanel: FC<PanelProps> = ({ ...panelProps }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();

	const navigationIndex = useNavigationIndex(projectId);

	const selection = useAppSelector((state) => selectProjectSelectionOutline(state, projectId));

	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);

	const dryRunOperation = operationMode
		? (getBinaryOperation({ mode: operationMode, target: selection }) ?? undefined)
		: undefined;

	// TODO: debounce?
	const dryRunOperationQuery = useDryRunOperation({ projectId, operation: dryRunOperation });
	const dryRunWorkspace = dryRunOperationQuery.data?.workspace ?? null;

	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));

	const openBranchPicker = () => {
		dispatch(projectActions.openBranchPicker({ projectId }));
	};

	useCommand(openBranchPicker, {
		group: "Outline",
		commandPalette: { label: "Select branch" },
		shortcutsBar: { label: "Branch" },
		hotkeys: [{ hotkey: "T" }],
	});

	return (
		<NavigationIndexContext value={navigationIndex}>
			<DryRunWorkspaceContext value={dryRunWorkspace}>
				<Panel
					{...panelProps}
					tabIndex={0}
					role="tree"
					aria-activedescendant={treeItemId(selection)}
					className={classes(panelProps.className, styles.panel)}
				>
					<div className={styles.panelPadding}>
						<Changes projectId={projectId} />
					</div>

					<div className={styles.scroller}>
						{headInfo.stacks.map((stack) => (
							<StackC key={stack.id} projectId={projectId} stack={stack} />
						))}

						<BaseCommit projectId={projectId} commitId={getCommonBaseCommitId(headInfo)} />
					</div>

					{Match.value(operationMode).pipe(
						Match.when(null, () => null),
						Match.tag("DragAndDrop", () => null),
						Match.orElse(({ source }) => (
							<div className={styles.operationModePreview}>
								<OperationSourceLabel headInfo={headInfo} source={source} />
							</div>
						)),
					)}
				</Panel>
			</DryRunWorkspaceContext>
		</NavigationIndexContext>
	);
};

const useIsSelected = ({ projectId, operand }: { projectId: string; operand: Operand }): boolean =>
	useAppSelector((state) => {
		const selection = selectProjectSelectionOutline(state, projectId);

		return operandEquals(selection, operand);
	});

const treeItemId = (operand: Operand): string =>
	`outline-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const ItemRow: FC<
	{
		projectId: string;
		operand: Operand;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ projectId, operand, ...props }) => {
	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsSelected({ projectId, operand });
	const selectItem = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};

	return (
		<WorkspaceItemRow
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, operand)}
			isSelected={isSelected}
			onSelect={selectItem}
		/>
	);
};

const TreeItem: FC<
	{
		projectId: string;
		operand: Operand;
		label: string;
		expanded?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, label, expanded, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(operand),
			role: "treeitem",
			"aria-label": label,
			"aria-selected": isSelected,
			"aria-expanded": expanded,
		}),
	});
};
const OperandC: FC<
	{
		projectId: string;
		operand: Operand;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				source={operand}
				render={
					<OperationTarget
						projectId={projectId}
						target={operand}
						isSelected={isSelected}
						render={render}
					/>
				}
			/>
		),
		defaultTagName: "div",
		props,
	});
};

const EditorHelp: FC<{
	hotkeys: Array<{ hotkey: string; name: string }>;
}> = ({ hotkeys }) => (
	<div className={styles.editorHelp}>
		{hotkeys.map((hotkey, index) => (
			<Fragment key={hotkey.hotkey}>
				{index > 0 && " • "}
				<kbd className={styles.editorShortcut}>{formatForDisplay(hotkey.hotkey)}</kbd>
				<span className={styles.editorShortcutLabel}> to {hotkey.name}</span>
			</Fragment>
		))}
	</div>
);

const InlineRewordCommit: FC<{
	message: string;
	onSubmit: (value: string) => void;
	onExit: () => void;
	projectId: string;
}> = ({ message, onSubmit, onExit, projectId }) => {
	const formRef = useRef<HTMLFormElement | null>(null);
	const focusedPanel = useFocusedProjectPanel(projectId);
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get("message") as string);
	};

	useCommand(() => formRef.current?.requestSubmit(), {
		group: "Reword commit",
		enabled: focusedPanel === "outline",
		shortcutsBar: { label: "Save" },
		hotkeys: [{ hotkey: "Enter", ignoreInputs: false }],
	});

	useCommand(onExit, {
		group: "Reword commit",
		enabled: focusedPanel === "outline",
		shortcutsBar: { label: "Cancel" },
		hotkeys: [{ hotkey: "Escape", ignoreInputs: false }],
	});

	return (
		<form ref={formRef} className={styles.editorForm} action={submitAction}>
			<textarea
				ref={(el) => {
					if (!el) return;
					el.focus();
					const cursorPosition = el.value.length;
					el.setSelectionRange(cursorPosition, cursorPosition);
				}}
				aria-label="Commit message"
				name="message"
				defaultValue={message.trim()}
				className={classes(styles.editorInput, styles.rewordCommitInput)}
			/>
			<EditorHelp
				hotkeys={[
					{ hotkey: "Enter", name: "Save" },
					{ hotkey: "Escape", name: "Cancel" },
				]}
			/>
		</form>
	);
};

const CommitRow: FC<
	{
		commit: Commit;
		projectId: string;
		stackId: string;
	} & ComponentProps<"div">
> = ({ commit, projectId, stackId, ...restProps }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isHighlighted = useAppSelector((state) =>
		selectProjectHighlightedCommitIds(state, projectId).includes(commit.id),
	);
	const dryRunCommit = useDryRunCommit(commit.id);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const dispatch = useAppDispatch();
	const commitOperandV: CommitOperand = {
		stackId,
		commitId: commit.id,
	};
	const operand = commitOperand(commitOperandV);
	const isSelected = useIsSelected({ projectId, operand });
	const isRewording =
		isSelected &&
		outlineMode._tag === "RewordCommit" &&
		operandEquals(operand, commitOperand(outlineMode.operand));
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);
	const [isCommitMessagePending, startCommitMessageTransition] = useTransition();

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};
	const { hasConflicts } = dryRunCommit ? dryRunCommit : commitWithOptimisticMessage;

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
	const commitReword = useMutation({
		...commitRewordMutationOptions,
		onSuccess: async (response, input, context, mutation) => {
			dispatch(
				projectActions.addReplacedCommits({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
				}),
			);

			await commitRewordMutationOptions.onSuccess?.(response, input, context, mutation);
		},
	});

	const insertBlankCommitAbove = () => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side: "above",
			dryRun: false,
		});
	};

	const insertBlankCommitBelow = () => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side: "below",
			dryRun: false,
		});
	};

	const deleteCommit = () => {
		commitDiscard.mutate({
			projectId,
			subjectCommitId: commit.id,
			dryRun: false,
		});
	};

	const runOperationMutation = useMutation(useRunOperationMutationOptions());

	const moveCommit = (offset: -1 | 1) => {
		const selectionIdx = navigationIndex.indexByKey.get(operandIdentityKey(operand));
		if (selectionIdx === undefined) return;

		const nextItem = navigationIndex.items[selectionIdx + offset];
		if (!nextItem) return;

		const operation = moveOperation({
			source: operand,
			target: nextItem,
			side: offset === -1 ? "above" : "below",
		});
		if (!operation) return;

		runOperationMutation.mutate(operation);
	};

	const cutCommit = () => {
		dispatch(projectActions.enterCutMode({ projectId, source: operand, operationType: "rub" }));
	};

	const startEditing = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		dispatch(projectActions.startRewordCommit({ projectId, commit: commitOperandV }));
	};
	const focusedPanel = useFocusedProjectPanel(projectId);

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusPanel("outline");
	};

	const saveNewMessage = (newMessage: string) => {
		const initialMessage = commit.message.trim();
		const trimmed = newMessage.trim();
		if (trimmed === initialMessage) return;
		startCommitMessageTransition(async () => {
			setOptimisticMessage(trimmed);
			try {
				await commitReword.mutateAsync({
					projectId,
					commitId: commit.id,
					message: trimmed,
					dryRun: false,
				});
			} catch {
				// Use the global mutation error handler (shows toast) instead of React
				// error boundaries.
				return;
			}
		});
	};

	const amendCommit = () => {
		dispatch(
			projectActions.enterCutMode({
				projectId,
				source: changesSectionOperand,
				operationType: "rub",
			}),
		);
		focusPanel("outline");
	};

	const { contextMenu: amendCommitContextMenuItem } = useCommand(amendCommit, {
		enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
		group: "Commit",
		commandPalette: { label: "Amend" },
		shortcutsBar: { label: "Amend" },
		contextMenu: {
			label: "Amend commit",
			// Focus change is too slow / the menu item isn't reactive.
			enabled: true,
		},
		hotkeys: [{ hotkey: "Shift+A" }],
	});

	const { contextMenu: cutCommitContextMenuItem } = useCommand(cutCommit, {
		enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
		group: "Commit",
		commandPalette: { label: "Cut" },
		// TODO: missing shortcut because it's defined elsewhere for all operands
		contextMenu: {
			label: "Cut commit",
			// Focus change is too slow / the menu item isn't reactive.
			enabled: true,
		},
	});

	const { contextMenu: startEditingContextMenuItem } = useCommand(startEditing, {
		enabled:
			!isCommitMessagePending &&
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default",
		group: "Commit",
		commandPalette: { label: "Reword" },
		shortcutsBar: { label: "Reword" },
		hotkeys: [{ hotkey: "Enter" }],
		contextMenu: {
			label: "Reword commit",
			enabled: !isCommitMessagePending,
		},
	});

	useCommand(() => moveCommit(-1), {
		enabled:
			!runOperationMutation.isPending &&
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default",
		group: "Commit",
		hotkeys: [{ hotkey: "Alt+ArrowUp" }],
	});

	useCommand(() => moveCommit(1), {
		enabled:
			!runOperationMutation.isPending &&
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default",
		group: "Commit",
		hotkeys: [{ hotkey: "Alt+ArrowDown" }],
	});

	const { contextMenu: insertBlankCommitAboveContextMenuItem } = useCommand(
		insertBlankCommitAbove,
		{
			enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
			group: "Commit",
			commandPalette: { label: "Add empty commit above" },
			contextMenu: {
				label: "Above",
				// Focus change is too slow / the menu item isn't reactive.
				enabled: true,
			},
		},
	);

	const { contextMenu: insertBlankCommitBelowContextMenuItem } = useCommand(
		insertBlankCommitBelow,
		{
			enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
			group: "Commit",
			commandPalette: { label: "Add empty commit below" },
			contextMenu: {
				label: "Below",
				// Focus change is too slow / the menu item isn't reactive.
				enabled: true,
			},
		},
	);

	const { contextMenu: deleteCommitContextMenuItem } = useCommand(deleteCommit, {
		enabled:
			!commitDiscard.isPending &&
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default",
		group: "Commit",
		commandPalette: { label: "Delete commit" },
		contextMenu: {
			label: "Delete commit",
			enabled: !commitDiscard.isPending,
		},
	});

	const menuItems: Array<NativeMenuItem> = [
		amendCommitContextMenuItem,
		cutCommitContextMenuItem,
		startEditingContextMenuItem,
		{
			_tag: "Item",
			label: "Add empty commit",
			submenu: [insertBlankCommitAboveContextMenuItem, insertBlankCommitBelowContextMenuItem],
		},
		deleteCommitContextMenuItem,
	];

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			className={classes(
				restProps.className,
				isHighlighted && workspaceItemRowStyles.itemRowHighlighted,
			)}
		>
			{isRewording ? (
				<InlineRewordCommit
					message={optimisticMessage}
					onSubmit={saveNewMessage}
					onExit={endEditing}
					projectId={projectId}
				/>
			) : (
				<>
					<div
						className={workspaceItemRowStyles.itemRowLabel}
						onDoubleClick={outlineMode._tag === "Default" ? startEditing : undefined}
						onContextMenu={
							outlineMode._tag === "Default"
								? (event) => {
										void showNativeContextMenu(event, menuItems);
									}
								: undefined
						}
					>
						{commitTitle(commitWithOptimisticMessage.message)}
						{hasConflicts && " ⚠️"}
					</div>
					{outlineMode._tag === "Default" && (
						<WorkspaceItemRowToolbar aria-label="Commit actions">
							<Toolbar.Button
								type="button"
								className={workspaceItemRowStyles.itemRowToolbarButton}
								aria-label="Commit menu"
								onClick={(event) => {
									void showNativeMenuFromTrigger(event.currentTarget, menuItems);
								}}
							>
								<MenuTriggerIcon />
							</Toolbar.Button>
						</WorkspaceItemRowToolbar>
					)}
				</>
			)}
		</ItemRow>
	);
};

const CommitC: FC<{
	commit: Commit;
	projectId: string;
	stackId: string;
}> = ({ commit, projectId, stackId }) => {
	const operand = commitOperand({ stackId, commitId: commit.id });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={commitTitle(commit.message)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<CommitRow commit={commit} projectId={projectId} stackId={stackId} />
		</TreeItem>
	);
};

const ChangesSectionRow: FC<{
	changes: Array<TreeChange>;

	projectId: string;
}> = ({ changes, projectId }) => {
	const operand = changesSectionOperand;
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		void queryClient
			.fetchQuery(absorptionPlanQueryOptions({ projectId, target: sourceTarget }))
			.then((absorptionPlan) => {
				dispatch(projectActions.enterAbsorbMode({ projectId, source, absorptionPlan }));
			});
	};

	const absorb = () => {
		enterAbsorbMode(operand, { type: "all" });
	};

	const { contextMenu: absorbContextMenuItem } = useCommand(absorb, {
		enabled:
			changes.length > 0 &&
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default",
		group: "Changes",
		commandPalette: { label: "Absorb" },
		shortcutsBar: { label: "Absorb" },
		hotkeys: [{ hotkey: "A" }],
		contextMenu: {
			label: "Absorb",
			enabled: changes.length > 0,
		},
	});

	const menuItems: Array<NativeMenuItem> = [absorbContextMenuItem];

	return (
		<ItemRow projectId={projectId} operand={operand}>
			<div
				className={classes(
					workspaceItemRowStyles.itemRowLabel,
					workspaceItemRowStyles.sectionLabel,
				)}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				Changes ({changes.length})
			</div>
			{outlineMode._tag === "Default" && (
				<WorkspaceItemRowToolbar aria-label="Changes actions">
					<Toolbar.Button
						type="button"
						className={workspaceItemRowStyles.itemRowToolbarButton}
						aria-label="Changes menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
					>
						<MenuTriggerIcon />
					</Toolbar.Button>
				</WorkspaceItemRowToolbar>
			)}
		</ItemRow>
	);
};

const BaseCommit: FC<{
	projectId: string;
	commitId?: string;
}> = ({ projectId, commitId }) => {
	const operand = baseCommitOperand;

	return (
		<div className={workspaceItemRowStyles.section}>
			<TreeItem
				projectId={projectId}
				operand={operand}
				label="Base commit"
				render={
					<OperandC
						projectId={projectId}
						operand={operand}
						render={<ItemRow projectId={projectId} operand={operand} />}
					/>
				}
			>
				<div
					className={classes(
						workspaceItemRowStyles.itemRowLabel,
						workspaceItemRowStyles.sectionLabel,
					)}
				>
					{commitId !== undefined
						? `${shortCommitId(commitId)} (common base commit)`
						: "(base commit)"}
				</div>
			</TreeItem>
		</div>
	);
};

type CommitBranchComboboxItem = {
	id: string;
	label: string;
	branch: BranchOperand;
};

const CommitBranchComboboxPopup: FC = () => (
	<Combobox.Popup className={classes(uiStyles.popup, styles.commitBranchComboboxPopup)}>
		<Combobox.Input
			aria-label="Search branches"
			placeholder="Search branches…"
			className={styles.commitBranchComboboxInput}
		/>
		<Combobox.Empty>
			<div className={styles.commitBranchComboboxEmpty}>No branches found.</div>
		</Combobox.Empty>
		<Combobox.List className={styles.commitBranchComboboxList}>
			{(item: CommitBranchComboboxItem) => (
				<Combobox.Item key={item.id} value={item} className={styles.commitBranchComboboxItem}>
					{item.label}
				</Combobox.Item>
			)}
		</Combobox.List>
	</Combobox.Popup>
);

const Changes: FC<{
	projectId: string;
}> = ({ projectId }) => {
	const toastManager = Toast.useToastManager();

	const commitCreate = useMutation({
		...commitCreateMutationOptions,
		onSuccess: async (response, input, context, mutation) => {
			await commitCreateMutationOptions.onSuccess?.(response, input, context, mutation);

			if (response.rejectedChanges.length > 0)
				toastManager.add(
					rejectedChangesToastOptions({
						newCommit: response.newCommit,
						rejectedChanges: response.rejectedChanges,
					}),
				);
		},
	});

	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const operand = changesSectionOperand;
	const commitTextareaRef = useRef<HTMLTextAreaElement | null>(null);
	const focusedPanel = useFocusedProjectPanel(projectId);
	const dispatch = useAppDispatch();

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const branchComboboxItems = headInfo.stacks.flatMap((stack): Array<CommitBranchComboboxItem> => {
		// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
		const stackId = stack.id!;
		return stack.segments.flatMap((segment): Array<CommitBranchComboboxItem> => {
			const refName = segment.refName;
			if (!refName) return [];

			return [
				{
					id: JSON.stringify([stackId, refName.fullNameBytes]),
					label: refName.displayName,
					branch: { stackId, branchRef: refName.fullNameBytes },
				},
			];
		});
	});

	const [branchId, setBranchId] = useState<string | null>(null);
	const branch = branchComboboxItems.find((item) => item.id === branchId) ?? branchComboboxItems[0];

	const changes = useResolveDiffSpecs({
		source: changesSectionOperand,
		projectId,
	});

	const commit = () => {
		if (!branch) return;

		if (!changes) return;

		commitCreate.mutate(
			{
				projectId,
				relativeTo: {
					type: "referenceBytes",
					subject: branch.branch.branchRef,
				},
				side: "below",
				changes,
				message: commitTextareaRef.current?.value ?? "",
				dryRun: false,
			},
			{
				onSuccess: (response) => {
					if (response.newCommit !== null && commitTextareaRef.current)
						commitTextareaRef.current.value = "";
				},
			},
		);
		focusPanel("outline");
	};

	const [open, setOpen] = useState(false);

	const selectBranch = (option: CommitBranchComboboxItem | null) => {
		setBranchId(option?.id ?? null);
		setOpen(false);
	};
	const openBranchCombobox = () => setOpen(true);

	const isSelected = useIsSelected({ projectId, operand });
	const selectChanges = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
	};
	const selectChangesAndFocusOutline = () => {
		selectChanges();
		focusPanel("outline");
	};
	const composeCommitMessage = () => {
		selectChanges();
		commitTextareaRef.current?.focus();
	};

	useCommand(selectChangesAndFocusOutline, {
		group: "Outline",
		commandPalette: { label: "Select changes" },
		shortcutsBar: { label: "Changes" },
		hotkeys: [{ hotkey: "Z" }],
	});

	useCommand(composeCommitMessage, {
		group: "Outline",
		commandPalette: { label: "Compose commit message" },
		shortcutsBar: { label: "Compose commit message" },
		hotkeys: [{ hotkey: "Shift+Z" }],
	});

	useCommand(() => commitTextareaRef.current?.focus(), {
		enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
		group: "Changes",
		commandPalette: { label: "Compose commit message" },
		shortcutsBar: { label: "Compose commit message" },
		hotkeys: [{ hotkey: "Enter" }],
	});

	const openBranchComboboxCommand = useCommand(openBranchCombobox, {
		enabled: outlineMode._tag === "Default",
		group: "Changes",
		commandPalette: { label: "Select commit branch" },
		shortcutsBar: { label: "Select commit branch" },
		hotkeys: [{ hotkey: "Mod+Shift+B" }],
	});

	const commitCommand = useCommand(commit, {
		enabled: outlineMode._tag === "Default" && !!branch,
		group: "Changes",
		commandPalette: { label: "Commit" },
		shortcutsBar: { label: "Commit" },
		hotkeys: [{ hotkey: "Mod+Enter" }],
	});

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={`Changes (${worktreeChanges.changes.length})`}
			className={classes(workspaceItemRowStyles.section, styles.changesSection)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<ChangesSectionRow changes={worktreeChanges.changes} projectId={projectId} />

			<textarea
				ref={commitTextareaRef}
				aria-label="Compose commit message"
				disabled={outlineMode._tag !== "Default"}
				placeholder="Commit message (optional)"
				className={styles.commitTextarea}
				onFocus={selectChanges}
				onKeyDown={(event) => {
					if (event.key !== "Escape") return;
					event.preventDefault();
					focusPanel("outline");
				}}
			/>

			<div className={styles.commitControls}>
				<Combobox.Root<CommitBranchComboboxItem>
					items={branchComboboxItems}
					open={open}
					onOpenChange={setOpen}
					// Note `undefined` means uncontrolled.
					value={branch ?? null}
					onValueChange={selectBranch}
					itemToStringLabel={(x) => x.label}
					itemToStringValue={(x) => x.id}
					isItemEqualToValue={(a, b) => a.id === b.id}
					autoHighlight
					disabled={outlineMode._tag !== "Default"}
				>
					<Combobox.Trigger
						className={classes(uiStyles.button, styles.commitBranchComboboxTrigger)}
						aria-label="Select branch"
						render={<ShortcutButton hotkeys={openBranchComboboxCommand.hotkeys} />}
					>
						<Combobox.Value placeholder="Select branch" />
					</Combobox.Trigger>
					<Combobox.Portal>
						<Combobox.Positioner align="start" sideOffset={8}>
							<CommitBranchComboboxPopup />
						</Combobox.Positioner>
					</Combobox.Portal>
				</Combobox.Root>

				<ShortcutButton
					hotkeys={commitCommand.hotkeys}
					className={classes(uiStyles.button, styles.changesSectionCommitButton)}
					onClick={commitCommand.commandFn}
					disabled={outlineMode._tag !== "Default" || !branch}
				>
					Commit
				</ShortcutButton>
			</div>
		</TreeItem>
	);
};

const InlineRenameBranch: FC<{
	branchName: string;
	onSubmit: (value: string) => void;
	onExit: () => void;
	projectId: string;
}> = ({ branchName, onSubmit, onExit, projectId }) => {
	const formRef = useRef<HTMLFormElement | null>(null);
	const focusedPanel = useFocusedProjectPanel(projectId);
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get("branchName") as string);
	};

	useCommand(() => formRef.current?.requestSubmit(), {
		group: "Rename branch",
		enabled: focusedPanel === "outline",
		shortcutsBar: { label: "Save" },
		hotkeys: [{ hotkey: "Enter", ignoreInputs: false }],
	});

	useCommand(onExit, {
		group: "Rename branch",
		enabled: focusedPanel === "outline",
		shortcutsBar: { label: "Cancel" },
		hotkeys: [{ hotkey: "Escape", ignoreInputs: false }],
	});

	return (
		<form ref={formRef} className={styles.editorForm} action={submitAction}>
			<input
				aria-label="Branch name"
				ref={(el) => {
					if (!el) return;
					el.focus();
					el.select();
				}}
				name="branchName"
				defaultValue={branchName}
				className={classes(styles.editorInput, styles.renameBranchInput)}
			/>
			<EditorHelp
				hotkeys={[
					{ hotkey: "Enter", name: "Save" },
					{ hotkey: "Escape", name: "Cancel" },
				]}
			/>
		</form>
	);
};

const BranchRow: FC<
	{
		projectId: string;
		branchName: string;
		branchRef: Array<number>;
		stackId: string;
	} & ComponentProps<"div">
> = ({ projectId, branchName, branchRef, stackId, ...restProps }) => {
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const dispatch = useAppDispatch();
	const branchOperandV: BranchOperand = {
		stackId,
		branchRef,
	};
	const operand = branchOperand(branchOperandV);
	const isRenaming =
		outlineMode._tag === "RenameBranch" &&
		operandEquals(operand, branchOperand(outlineMode.operand));
	const [optimisticBranchName, setOptimisticBranchName] = useOptimistic(
		branchName,
		(_currentBranchName, nextBranchName: string) => nextBranchName,
	);
	const [isRenamePending, startRenameTransition] = useTransition();

	const updateBranchName = useMutation({
		...updateBranchNameMutationOptions,
		onSuccess: async (response, input, context, mutation) => {
			await updateBranchNameMutationOptions.onSuccess?.(response, input, context, mutation);

			const newSelection = branchOperand({
				stackId,
				// TODO: ideally the API would return the new ref?
				branchRef: encodeRefName(`refs/heads/${input.newName}`),
			});
			dispatch(projectActions.selectOutline({ projectId, selection: newSelection }));
			dispatch(projectActions.exitMode({ projectId }));
		},
	});

	const startEditing = () => {
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		dispatch(projectActions.startRenameBranch({ projectId, branch: branchOperandV }));
	};
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectOutline({ projectId, selection: operand }));
		focusPanel("outline");
	};

	const saveBranchName = (newBranchName: string) => {
		const trimmed = newBranchName.trim();
		if (trimmed === "" || trimmed === branchName) return;
		startRenameTransition(async () => {
			setOptimisticBranchName(trimmed);
			try {
				await updateBranchName.mutateAsync({
					projectId,
					stackId,
					branchName,
					newName: trimmed,
				});
			} catch {
				// Use the global mutation error handler (shows toast) instead of React
				// error boundaries.
				return;
			}
		});
	};

	const { contextMenu: startEditingContextMenuItem } = useCommand(startEditing, {
		enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
		group: "Branch",
		commandPalette: { label: "Rename" },
		shortcutsBar: { label: "Rename" },
		hotkeys: [{ hotkey: "Enter" }],
		contextMenu: {
			label: "Rename branch",
			enabled: !isRenamePending,
		},
	});

	const menuItems: Array<NativeMenuItem> = [startEditingContextMenuItem];

	return (
		<ItemRow {...restProps} projectId={projectId} operand={operand}>
			{isRenaming ? (
				<InlineRenameBranch
					branchName={optimisticBranchName}
					onSubmit={saveBranchName}
					onExit={endEditing}
					projectId={projectId}
				/>
			) : (
				<>
					<div
						className={classes(
							workspaceItemRowStyles.itemRowLabel,
							workspaceItemRowStyles.sectionLabel,
						)}
						onDoubleClick={outlineMode._tag === "Default" ? startEditing : undefined}
						onContextMenu={
							outlineMode._tag === "Default"
								? (event) => {
										void showNativeContextMenu(event, menuItems);
									}
								: undefined
						}
					>
						{optimisticBranchName}
					</div>
					{outlineMode._tag === "Default" && (
						<WorkspaceItemRowToolbar aria-label="Branch actions">
							<Toolbar.Button
								type="button"
								className={workspaceItemRowStyles.itemRowToolbarButton}
								aria-label="Push branch"
								disabled
							>
								<PushIcon />
							</Toolbar.Button>
							<Toolbar.Button
								type="button"
								className={workspaceItemRowStyles.itemRowToolbarButton}
								aria-label="Branch menu"
								onClick={(event) => {
									void showNativeMenuFromTrigger(event.currentTarget, menuItems);
								}}
							>
								<MenuTriggerIcon />
							</Toolbar.Button>
						</WorkspaceItemRowToolbar>
					)}
				</>
			)}
		</ItemRow>
	);
};

const StackRow: FC<
	{
		projectId: string;
		stackId: string;
	} & ComponentProps<"div">
> = ({ projectId, stackId, ...restProps }) => {
	const operand = stackOperand({ stackId });
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const unapplyStack = useMutation(unapplyStackMutationOptions);
	const unapply = () => {
		unapplyStack.mutate({ projectId, stackId });
	};

	const { contextMenu: unapplyContextMenuItem } = useCommand(unapply, {
		enabled:
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default" &&
			!unapplyStack.isPending,
		group: "Stack",
		commandPalette: { label: "Unapply stack" },
		contextMenu: {
			label: "Unapply stack",
			// Focus change is too slow / the menu item isn't reactive.
			enabled: !unapplyStack.isPending,
		},
	});

	const menuItems: Array<NativeMenuItem> = [
		{ _tag: "Item", label: "Move up", enabled: false },
		{ _tag: "Item", label: "Move down", enabled: false },
		{ _tag: "Separator" },
		unapplyContextMenuItem,
	];

	return (
		<ItemRow {...restProps} projectId={projectId} operand={operand}>
			<div
				className={classes(
					workspaceItemRowStyles.itemRowLabel,
					workspaceItemRowStyles.sectionLabel,
				)}
				onContextMenu={
					outlineMode._tag === "Default"
						? (event) => {
								void showNativeContextMenu(event, menuItems);
							}
						: undefined
				}
			>
				Stack
			</div>
			{outlineMode._tag === "Default" && (
				<WorkspaceItemRowToolbar aria-label="Stack actions">
					<Toolbar.Button
						type="button"
						className={workspaceItemRowStyles.itemRowToolbarButton}
						aria-label="Stack menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
					>
						<MenuTriggerIcon />
					</Toolbar.Button>
				</WorkspaceItemRowToolbar>
			)}
		</ItemRow>
	);
};

const BranchSegment: FC<{
	projectId: string;
	segment: Segment;
	refName: BranchReference;
	stackId: string;
}> = ({ projectId, segment, refName, stackId }) => {
	const operand = branchOperand({ stackId, branchRef: refName.fullNameBytes });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={refName.displayName}
			expanded
			className={classes(workspaceItemRowStyles.section, styles.segment)}
		>
			<OperandC
				projectId={projectId}
				operand={operand}
				render={
					<BranchRow
						projectId={projectId}
						branchName={refName.displayName}
						branchRef={refName.fullNameBytes}
						stackId={stackId}
					/>
				}
			/>

			{segment.commits.length === 0 ? (
				<div className={workspaceItemRowStyles.itemRowEmpty}>No commits.</div>
			) : (
				<div role="group">
					{segment.commits.map((commit) => (
						<CommitC key={commit.id} commit={commit} projectId={projectId} stackId={stackId} />
					))}
				</div>
			)}
		</TreeItem>
	);
};

const BranchlessSegment: FC<{
	projectId: string;
	segment: Segment;
	stackId: string;
}> = ({ projectId, segment, stackId }) => (
	<div className={classes(workspaceItemRowStyles.section, styles.segment)}>
		{segment.commits.map((commit) => (
			<CommitC key={commit.id} commit={commit} projectId={projectId} stackId={stackId} />
		))}
	</div>
);

const StackC: FC<{
	projectId: string;
	stack: Stack;
}> = ({ projectId, stack }) => {
	// From Caleb:
	// > There shouldn't be a way within GitButler to end up with a stack without a
	//   StackId. Users can disrupt our matching against our metadata by playing
	//   with references, but we currently also try to patch it up at certain points
	//   so it probably isn't too common.
	// For now we'll treat this as non-nullable until we identify cases where it
	// could genuinely be null (assuming backend correctness).
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [tag:stack-id-required]
	const stackId = stack.id!;
	const operand = stackOperand({ stackId });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label="Stack"
			expanded
			className={classes(styles.stack, workspaceItemRowStyles.section)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<StackRow projectId={projectId} stackId={stackId} />

			<div role="group" className={styles.segments}>
				{stack.segments.map((segment) =>
					segment.refName ? (
						<BranchSegment
							key={JSON.stringify(segment.refName.fullNameBytes)}
							projectId={projectId}
							segment={segment}
							refName={segment.refName}
							stackId={stackId}
						/>
					) : (
						// A segment should always either have a branch reference or at
						// least one commit.
						isNonEmptyArray(segment.commits) && (
							<BranchlessSegment
								key={segment.commits[0].id}
								projectId={projectId}
								segment={segment}
								stackId={stackId}
							/>
						)
					),
				)}
			</div>
		</TreeItem>
	);
};
