import {
	applyBranchMutationOptions,
	commitDiscardMutationOptions,
	commitInsertBlankMutationOptions,
	commitRewordMutationOptions,
	unapplyStackMutationOptions,
	updateBranchNameMutationOptions,
} from "#ui/api/mutations.ts";
import {
	absorptionPlanQueryOptions,
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { classes } from "#ui/ui/classes.ts";
import {
	branchFileParent,
	changesFileParent,
	commitFileParent,
	type FileParent,
} from "#ui/operands.ts";
import { getBranchNameByCommitId, getCommonBaseCommitId } from "#ui/api/ref-info.ts";
import { useActiveElement } from "#ui/focus.ts";
import { DependencyIcon, ExpandCollapseIcon, MenuTriggerIcon, PushIcon } from "#ui/ui/icons.tsx";
import {
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { CommitLabel } from "#ui/routes/project/$id/CommitLabel.tsx";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { decodeRefName, encodeRefName } from "#ui/api/ref-name.ts";
import { orderedPanels, Panel as PanelType } from "#ui/panels.ts";
import { isPanelVisible } from "#ui/panels/state.ts";
import {
	projectActions,
	selectProjectExpandedCommitId,
	selectProjectHighlightedCommitIds,
	selectProjectPanelsState,
	selectProjectOperationModeState,
	selectProjectPickerDialogState,
	selectProjectSelection,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { AbsorptionDialog } from "#ui/routes/project/$id/workspace/AbsorptionDialog.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { OperationSourceLabel } from "#ui/routes/project/$id/workspace/OperationSourceLabel.tsx";
import { OperationTarget } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { ShortcutsBarPortal, TopBarActionsPortal } from "#ui/portals.tsx";
import { ShortcutButton } from "#ui/ui/ShortcutButton.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { isInputElement } from "#ui/commands/hotkeys.ts";
import uiStyles from "#ui/ui/ui.module.css";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import {
	AbsorptionTarget,
	BranchListing,
	Commit,
	DiffHunk,
	HunkDependencies,
	HunkHeader,
	Segment,
	Stack,
	TreeChange,
	UnifiedPatch,
} from "@gitbutler/but-sdk";
import { PatchDiff } from "@pierre/diffs/react";
import {
	formatForDisplay,
	getHotkeyManager,
	useHotkey,
	useHotkeyRegistrations,
	useHotkeySequence,
	useHotkeys,
	type HotkeyRegistrationView,
} from "@tanstack/react-hotkeys";
import {
	useMutation,
	useQuery,
	useQueryClient,
	useSuspenseQueries,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Match, pipe } from "effect";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";
import {
	ComponentProps,
	FC,
	Fragment,
	Ref,
	Suspense,
	useEffect,
	useOptimistic,
	useRef,
	useState,
	useTransition,
} from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import {
	baseCommitOperand,
	branchOperand,
	changesSectionOperand,
	commitOperand,
	fileOperand,
	hunkOperand,
	operandEquals,
	operandIdentityKey,
	stackOperand,
	type BranchOperand,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import { formatHunkHeader } from "#ui/hunk.ts";
import { PickerDialog, type PickerDialogGroup } from "#ui/ui/PickerDialog/PickerDialog.tsx";
import styles from "./WorkspacePage.module.css";
import { includeOperandForOutlineMode, isValidOutlineMode } from "#ui/outline/mode.ts";
import {
	buildNavigationIndex,
	filterNavigationIndex,
	getAdjacent,
	getNextSection,
	getPreviousSection,
	navigationIndexIncludes,
	type NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { useWorkspaceOutline } from "#ui/workspace/outline.ts";

const assert = <T,>(t: T | null | undefined): T => {
	if (t == null) throw new Error("Expected value to be non-null and defined");
	return t;
};

const getFocusedProjectPanel = (activeElement: Element | null) =>
	(activeElement?.closest("[data-panel]")?.id as PanelType | undefined) ?? null;

const useFocusedProjectPanel = (projectId: string): PanelType | null => {
	const activeElement = useActiveElement();
	const focusedPanel = getFocusedProjectPanel(activeElement);
	const pickerDialog = useAppSelector((state) => selectProjectPickerDialogState(state, projectId));
	return pickerDialog._tag === "CommandPalette" ? pickerDialog.focusedPanel : focusedPanel;
};

const useProjectPanelFocusManager = () => {
	const panelElementsRef = useRef(new Map<PanelType, HTMLDivElement>());
	const panelElementRef =
		(panel: PanelType) =>
		(element: HTMLDivElement | null): void => {
			if (element) panelElementsRef.current.set(panel, element);
			else panelElementsRef.current.delete(panel);
		};
	const focusPanel = (panel: PanelType) => {
		panelElementsRef.current.get(panel)?.focus({ focusVisible: false });
	};
	const focusAdjacentPanel = (offset: -1 | 1) => {
		const currentPanel = getFocusedProjectPanel(document.activeElement);
		if (currentPanel === null) return;
		const nextPanel = orderedPanels[orderedPanels.indexOf(currentPanel) + offset];
		if (nextPanel === undefined) return;
		focusPanel(nextPanel);
	};

	return {
		focusAdjacentPanel,
		focusPanel,
		panelElementRef,
	};
};

const OutlinePanel: FC<{
	elementRef: Ref<HTMLDivElement | null>;
	focusPanel: (panel: PanelType) => void;
	navigationIndex: NavigationIndex;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
}> = ({ elementRef, focusPanel, navigationIndex, onAbsorbChanges }) => {
	const dispatch = useAppDispatch();
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);
	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);
	const commit = () =>
		dispatch(
			projectActions.enterMoveMode({
				projectId,
				source: changesSectionOperand,
			}),
		);

	useOutlineSelectionHotkeys({
		focusedPanel,
		navigationIndex,
		projectId,
	});

	return (
		<Panel
			id={"outline" satisfies PanelType}
			minSize={400}
			elementRef={elementRef}
			tabIndex={-1}
			role="tree"
			className={classes(styles.panel, styles.outlinePanel)}
		>
			<Changes
				projectId={projectId}
				onAbsorbChanges={onAbsorbChanges}
				onCommit={commit}
				navigationIndex={navigationIndex}
			/>

			{headInfo.stacks.map((stack) => (
				<StackC
					key={stack.id}
					projectId={projectId}
					stack={stack}
					navigationIndex={navigationIndex}
					focusPanel={focusPanel}
				/>
			))}

			<BaseCommit
				projectId={projectId}
				commitId={getCommonBaseCommitId(headInfo)}
				navigationIndex={navigationIndex}
			/>

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
	);
};

const DetailsPanel: FC<{
	elementRef: Ref<HTMLDivElement | null>;
	focusPanel: (panel: PanelType) => void;
}> = ({ elementRef, focusPanel }) => {
	const dispatch = useAppDispatch();
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const selection = useAppSelector((state) => selectProjectSelection(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);

	useHotkey(
		"Escape",
		() => {
			dispatch(projectActions.hidePanel({ projectId, panel: "details" }));
			focusPanel("outline");
		},
		{
			conflictBehavior: "allow",
			enabled: focusedPanel === "details",
			meta: { group: "Details", name: "Close" },
		},
	);

	return (
		<Panel
			id={"details" satisfies PanelType}
			minSize={300}
			defaultSize="70%"
			elementRef={elementRef}
			tabIndex={0}
			className={styles.panel}
		>
			<Suspense fallback={<div>Loading details…</div>}>
				<Details projectId={projectId} selection={selection} />
			</Suspense>
		</Panel>
	);
};

type HotkeyGroup =
	| "Branch"
	| "Branches"
	| "Changes file"
	| "Changes"
	| "Commit file"
	| "Commit"
	| "Details"
	| "Global"
	| "Outline selection"
	| "Operation mode"
	| "Panels"
	| "Rename branch"
	| "Reword commit"
	| "Stack";

declare module "@tanstack/react-hotkeys" {
	interface HotkeyMeta {
		/**
		 * The component where the hotkey is registered.
		 */
		group: HotkeyGroup;
		/**
		 * @default true
		 *
		 * Whether or not to display the command and/or hotkey in the command palette.
		 */
		commandPalette?: boolean | "hideHotkey";
		/**
		 * @default true
		 *
		 * Whether or not to display the command and associated hotkey in the shortcuts bar.
		 */
		shortcutsBar?: boolean;
	}
}

type HunkDependencyDiff = HunkDependencies["diffs"][number];

const useIsSelected = ({ projectId, operand }: { projectId: string; operand: Operand }): boolean =>
	useAppSelector((state) => {
		const selection = selectProjectSelection(state, projectId);

		return operandEquals(selection, operand);
	});

const treeItemId = (projectId: string, operand: Operand): string =>
	`project-${encodeURIComponent(projectId)}-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const focusTreeItem = (projectId: string, operand: Operand) => {
	document.getElementById(treeItemId(projectId, operand))?.focus();
};

const useOutlineSelectionHotkeys = ({
	focusedPanel,
	navigationIndex,
	projectId,
}: {
	focusedPanel: PanelType | null;
	navigationIndex: NavigationIndex;
	projectId: string;
}) => {
	const dispatch = useAppDispatch();
	const selection = useAppSelector((state) => selectProjectSelection(state, projectId));

	const selectAndFocus = (newItem: Operand) => {
		dispatch(projectActions.select({ projectId, selection: newItem }));
		focusTreeItem(projectId, newItem);
	};

	const moveSelection = (offset: -1 | 1) => {
		const newItem = getAdjacent({ navigationIndex, selection, offset });
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectPreviousItem = () => {
		moveSelection(-1);
	};

	const selectNextItem = () => {
		moveSelection(1);
	};

	const selectNextSection = () => {
		const newItem = getNextSection({ navigationIndex, selection });
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectPreviousSection = () => {
		const newItem = getPreviousSection({ navigationIndex, selection });
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectChanges = () => {
		selectAndFocus(changesSectionOperand);
	};

	const selectFirstItem = () => {
		const newItem = navigationIndex.items[0];
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectLastItem = () => {
		const newItem = navigationIndex.items.at(-1);
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const openBranchPicker = () => {
		dispatch(projectActions.openBranchPicker({ projectId }));
	};

	const enterMoveMode = () => {
		dispatch(projectActions.enterMoveMode({ projectId, source: selection }));
	};

	const enterRubMode = () => {
		dispatch(projectActions.enterRubMode({ projectId, source: selection }));
	};

	const enterCommitMode = () => {
		dispatch(projectActions.enterMoveMode({ projectId, source: changesSectionOperand }));
	};

	useHotkeys(
		[
			{
				hotkey: "ArrowUp",
				callback: selectPreviousItem,
				options: { meta: { group: "Outline selection", name: "Up", commandPalette: false } },
			},
			{
				hotkey: "K",
				callback: selectPreviousItem,
				// Hidden until we can combine in shortcuts bar.
				options: { meta: { group: "Outline selection", shortcutsBar: false } },
			},
			{
				hotkey: "ArrowDown",
				callback: selectNextItem,
				options: { meta: { group: "Outline selection", name: "Down", commandPalette: false } },
			},
			{
				hotkey: "J",
				callback: selectNextItem,
				// Hidden until we can combine in shortcuts bar.
				options: { meta: { group: "Outline selection", shortcutsBar: false } },
			},
			{
				hotkey: "Shift+ArrowUp",
				callback: selectPreviousSection,
				options: {
					meta: {
						group: "Outline selection",
						name: "Previous section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+K",
				callback: selectPreviousSection,
				options: {
					meta: {
						group: "Outline selection",
						name: "Previous section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+ArrowDown",
				callback: selectNextSection,
				options: {
					meta: {
						group: "Outline selection",
						name: "Next section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+J",
				callback: selectNextSection,
				options: {
					meta: {
						group: "Outline selection",
						name: "Next section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Home",
				callback: selectFirstItem,
				options: {
					meta: {
						group: "Outline selection",
						name: "First item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Meta+ArrowUp",
				callback: selectFirstItem,
				options: {
					meta: {
						group: "Outline selection",
						name: "First item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "End",
				callback: selectLastItem,
				options: {
					meta: {
						group: "Outline selection",
						name: "Last item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Meta+ArrowDown",
				callback: selectLastItem,
				options: {
					meta: {
						group: "Outline selection",
						name: "Last item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+G",
				callback: selectLastItem,
				options: {
					meta: {
						group: "Outline selection",
						name: "Last item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
		],
		{ enabled: focusedPanel === "outline" },
	);

	useHotkeySequence(["G", "G"], selectFirstItem, {
		enabled: focusedPanel === "outline",
		meta: {
			group: "Outline selection",
			name: "First item",
			commandPalette: false,
			shortcutsBar: false,
		},
	});

	useHotkeys([
		{
			hotkey: "T",
			callback: openBranchPicker,
			options: { meta: { group: "Outline selection", name: "Branch" } },
		},
		{
			hotkey: "Z",
			callback: selectChanges,
			options: { meta: { group: "Outline selection", name: "Changes" } },
		},
	]);

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	useHotkeys(
		[
			{
				hotkey: "M",
				callback: enterMoveMode,
				options: { meta: { group: "Outline selection", name: "Move" } },
			},
			{
				hotkey: "Mod+X",
				callback: enterMoveMode,
				options: {
					ignoreInputs: true,
					meta: { group: "Outline selection", name: "Cut" },
				},
			},
			{
				hotkey: "R",
				callback: enterRubMode,
				options: { meta: { group: "Outline selection", name: "Rub" } },
			},
			{
				hotkey: "C",
				callback: enterCommitMode,
				options: { meta: { group: "Outline selection", name: "Commit" } },
			},
		],
		{ enabled: focusedPanel === "outline" && outlineMode._tag === "Default" },
	);
};

const lineEndingForDiff = (diff: string): string => (diff.includes("\r\n") ? "\r\n" : "\n");

const patchHeaderForChange = (change: TreeChange, lineEnding: string): string =>
	Match.value(change.status).pipe(
		Match.when(
			{ type: "Addition" },
			() => `--- /dev/null${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.when(
			{ type: "Deletion" },
			() => `--- ${change.path}${lineEnding}+++ /dev/null${lineEnding}`,
		),
		Match.when(
			{ type: "Modification" },
			() => `--- ${change.path}${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.when(
			{ type: "Rename" },
			({ subject }) => `--- ${subject.previousPath}${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.exhaustive,
	);

const HunkDiff: FC<{
	change: TreeChange;
	diff: string;
}> = ({ change, diff }) => (
	<PatchDiff
		patch={`${patchHeaderForChange(change, lineEndingForDiff(diff))}${diff}`}
		options={{
			diffStyle: "unified",
			themeType: "system",
			disableFileHeader: true,
		}}
	/>
);

const hunkKey = (hunk: HunkHeader): string =>
	`${hunk.oldStart}:${hunk.oldLines}:${hunk.newStart}:${hunk.newLines}`;

const fileRowLabel = (change: TreeChange) => {
	const status = Match.value(change.status).pipe(
		Match.when({ type: "Addition" }, () => "A"),
		Match.when({ type: "Deletion" }, () => "D"),
		Match.when({ type: "Modification" }, () => "M"),
		Match.when({ type: "Rename" }, () => "R"),
		Match.exhaustive,
	);

	return `${status} ${change.path}`;
};

const CommitFiles: FC<{
	projectId: string;
	commitId: string;
	parentCommitOperand: CommitOperand;
	navigationIndex: NavigationIndex;
}> = ({ projectId, commitId, parentCommitOperand, navigationIndex }) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	const conflictedPaths = data.conflictEntries
		? globalThis.Array.from(
				new Set([
					...data.conflictEntries.ancestorEntries,
					...data.conflictEntries.ourEntries,
					...data.conflictEntries.theirEntries,
				]),
			).sort((a: string, b: string) => a.localeCompare(b))
		: [];

	if (conflictedPaths.length === 0 && data.changes.length === 0)
		return <div className={styles.itemRowEmpty}>No file changes.</div>;

	return (
		<>
			{conflictedPaths.length > 0 && (
				<div>
					<div>Conflicts:</div>
					<ul>
						{conflictedPaths.map((path: string) => (
							<li key={path}>{path}</li>
						))}
					</ul>
				</div>
			)}

			{data.changes.length > 0 && (
				<div role="group">
					{data.changes.map((file) => (
						<CommitFileRow
							key={file.path}
							change={file}
							parentCommitOperand={parentCommitOperand}
							navigationIndex={navigationIndex}
							projectId={projectId}
						/>
					))}
				</div>
			)}
		</>
	);
};

const ItemRowPresentational: FC<
	{
		isSelected?: boolean;
	} & ComponentProps<"div">
> = ({ className, isSelected, ...props }) => (
	<div
		{...props}
		className={classes(className, styles.itemRow, isSelected && styles.itemRowSelected)}
	/>
);

const ItemRow: FC<
	{
		projectId: string;
		operand: Operand;
		navigationIndex: NavigationIndex;
	} & Omit<ComponentProps<typeof ItemRowPresentational>, "inert" | "isSelected">
> = ({ projectId, operand, navigationIndex, onClick, ...props }) => {
	const dispatch = useAppDispatch();
	const isSelected = useIsSelected({ projectId, operand });

	return (
		<ItemRowPresentational
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, operand)}
			isSelected={isSelected}
			onClick={(event) => {
				onClick?.(event);
				if (!event.defaultPrevented)
					dispatch(projectActions.select({ projectId, selection: operand }));
			}}
		/>
	);
};

const ItemRowToolbar: FC<Omit<ComponentProps<typeof Toolbar.Root>, "className">> = ({
	onClick,
	...props
}) => (
	<Toolbar.Root
		{...props}
		className={styles.itemRowToolbar}
		onClick={(event) => {
			onClick?.(event);
			event.stopPropagation();
		}}
	/>
);

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
			id: treeItemId(projectId, operand),
			role: "treeitem",
			tabIndex: isSelected ? 0 : -1,
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
						operand={operand}
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

const DependencyIndicatorButton: FC<
	{
		projectId: string;
		commitIds: NonEmptyArray<string>;
	} & useRender.ComponentProps<"button">
> = ({ projectId, commitIds, ...restProps }) => {
	// We use a controlled tooltip as a workaround for https://github.com/mui/base-ui/issues/4499.
	const [isTooltipOpen, setIsTooltipOpen] = useState(false);
	const dispatch = useAppDispatch();
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	// TODO: expensive
	const branchNameByCommitId = getBranchNameByCommitId(headInfo);
	const branchNames = pipe(
		commitIds,
		Array.flatMapNullable((commitId) => branchNameByCommitId.get(commitId)),
		Array.dedupe,
	);
	const tooltip =
		branchNames.length > 0 ? `Depends on ${branchNames.join(", ")}` : "Unknown dependencies";
	const highlightCommitIds = () => {
		setIsTooltipOpen(true);
		dispatch(
			projectActions.setHighlightedCommitIds({
				projectId,
				commitIds,
			}),
		);
	};
	const clearHighlightedCommitIds = () => {
		setIsTooltipOpen(false);
		dispatch(projectActions.setHighlightedCommitIds({ projectId, commitIds: null }));
	};

	return (
		<Tooltip.Root
			open={isTooltipOpen}
			// [ref:tooltip-disable-hoverable-popup]
			disableHoverablePopup
		>
			<Tooltip.Trigger
				{...restProps}
				type="button"
				onMouseEnter={highlightCommitIds}
				onMouseLeave={clearHighlightedCommitIds}
				onFocus={highlightCommitIds}
				onBlur={clearHighlightedCommitIds}
				aria-label={tooltip}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
						{tooltip}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const hunkContainsHunk = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart <= b.oldStart &&
	a.oldStart + a.oldLines - 1 >= b.oldStart + b.oldLines - 1 &&
	a.newStart <= b.newStart &&
	a.newStart + a.newLines - 1 >= b.newStart + b.newLines - 1;

const getHunkDependencyDiffsByPath = (
	hunkDependencyDiffs: Array<HunkDependencyDiff>,
): Map<string, Array<HunkDependencyDiff>> => {
	const byPath = new Map<string, Array<HunkDependencyDiff>>();

	for (const hunkDependencyDiff of hunkDependencyDiffs) {
		const [path] = hunkDependencyDiff;
		const pathDependencyDiffs = byPath.get(path);
		if (pathDependencyDiffs) pathDependencyDiffs.push(hunkDependencyDiff);
		else byPath.set(path, [hunkDependencyDiff]);
	}

	return byPath;
};

const getDependencyCommitIds = ({
	hunk,
	hunkDependencyDiffs,
}: {
	hunk?: DiffHunk;
	hunkDependencyDiffs: Array<HunkDependencyDiff>;
}): NonEmptyArray<string> | undefined => {
	const commitIds = new Set<string>();

	for (const [, dependencyHunk, locks] of hunkDependencyDiffs) {
		if (hunk && !hunkContainsHunk(hunk, dependencyHunk)) continue;
		for (const dependency of locks) commitIds.add(dependency.commitId);
	}

	const dependencyCommitIds = globalThis.Array.from(commitIds);
	return isNonEmptyArray(dependencyCommitIds) ? dependencyCommitIds : undefined;
};

const Hunk: FC<{
	isResultOfBinaryToTextConversion: boolean;
	projectId: string;
	fileParent: FileParent;
	change: TreeChange;
	hunk: DiffHunk;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
}> = ({
	isResultOfBinaryToTextConversion,
	projectId,
	fileParent,
	change,
	hunk,
	hunkDependencyDiffs,
}) => {
	const dependencyCommitIds =
		fileParent._tag === "Changes" && hunkDependencyDiffs
			? getDependencyCommitIds({ hunk, hunkDependencyDiffs })
			: undefined;

	const operand = hunkOperand({
		parent: fileParent,
		path: change.path,
		hunkHeader: hunk,
		isResultOfBinaryToTextConversion,
	});

	return (
		<div>
			<OperationSourceC projectId={projectId} source={operand}>
				<div className={styles.hunkHeaderRow}>
					{dependencyCommitIds && (
						<DependencyIndicatorButton projectId={projectId} commitIds={dependencyCommitIds}>
							<DependencyIcon />
						</DependencyIndicatorButton>
					)}
					<div className={styles.hunkHeader}>{formatHunkHeader(hunk)}</div>
				</div>
			</OperationSourceC>
			<HunkDiff change={change} diff={hunk.diff} />
		</div>
	);
};

const FileDiff: FC<{
	projectId: string;
	change: TreeChange;
	fileParent: FileParent;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
	diff: UnifiedPatch | null;
}> = ({ projectId, change, fileParent, hunkDependencyDiffs, diff }) =>
	Match.value(diff).pipe(
		Match.when(null, () => <div>No diff available for this file.</div>),
		Match.when({ type: "Binary" }, () => <div>Binary file (diff not available).</div>),
		Match.when({ type: "TooLarge" }, ({ subject }) => (
			<div>Diff too large ({subject.sizeInBytes} bytes).</div>
		)),
		Match.when({ type: "Patch" }, (patch) => {
			const { hunks } = patch.subject;
			if (hunks.length === 0) return <div>No hunks.</div>;

			return (
				<ul>
					{hunks.map((hunk) => (
						<li key={hunkKey(hunk)}>
							<Hunk
								isResultOfBinaryToTextConversion={patch.subject.isResultOfBinaryToTextConversion}
								projectId={projectId}
								fileParent={fileParent}
								change={change}
								hunk={hunk}
								hunkDependencyDiffs={hunkDependencyDiffs}
							/>
						</li>
					))}
				</ul>
			);
		}),
		Match.exhaustive,
	);

const ChangesFileDiffList: FC<{
	changes: Array<TreeChange>;
	projectId: string;
	fileParent: FileParent;
	hunkDependencyDiffsByPath?: Map<string, Array<HunkDependencyDiff>>;
}> = ({ changes, projectId, fileParent, hunkDependencyDiffsByPath }) => {
	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(changes, Array.zip(treeChangeDiffs));

	return changesWithDiffs.length === 0 ? (
		<div>No file changes.</div>
	) : (
		<ul>
			{changesWithDiffs.map(([change, diff]) => {
				const source = fileOperand({ parent: fileParent, path: change.path });

				return (
					<li key={change.path}>
						<OperationSourceC projectId={projectId} source={source}>
							<h4>{change.path}</h4>
						</OperationSourceC>
						<FileDiff
							projectId={projectId}
							change={change}
							fileParent={fileParent}
							hunkDependencyDiffs={hunkDependencyDiffsByPath?.get(change.path)}
							diff={diff}
						/>
					</li>
				);
			})}
		</ul>
	);
};

const ChangesDetails: FC<{
	projectId: string;
	selectedPath?: string;
}> = ({ projectId, selectedPath }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);
	const selectedChange =
		selectedPath !== undefined
			? worktreeChanges.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : worktreeChanges.changes;

	return (
		<div>
			<ChangesFileDiffList
				changes={changes}
				fileParent={changesFileParent}
				hunkDependencyDiffsByPath={hunkDependencyDiffsByPath}
				projectId={projectId}
			/>
		</div>
	);
};

const CommitDetails: FC<{
	projectId: string;
	commitId: string;
	selectedPath?: string | null;
	stackId: string;
}> = ({ projectId, commitId, selectedPath, stackId }) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	const selectedChange =
		selectedPath !== undefined
			? commitDetails.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : commitDetails.changes;
	const fileParent = commitFileParent({ stackId, commitId });

	return (
		<div>
			{selectedPath === undefined && (
				<>
					<h3>
						<CommitLabel commit={commitDetails.commit} />
					</h3>
					{commitDetails.commit.message.includes("\n") && (
						<p className={styles.commitMessageBody}>
							{commitDetails.commit.message
								.slice(commitDetails.commit.message.indexOf("\n") + 1)
								.trim()}
						</p>
					)}
				</>
			)}
			<ChangesFileDiffList changes={changes} fileParent={fileParent} projectId={projectId} />
		</div>
	);
};

const BranchDetails: FC<{
	projectId: string;
	branchRef: Array<number>;
	selectedPath?: string;
	stackId: string;
}> = ({ projectId, branchRef, selectedPath, stackId }) => {
	const decodedBranchRef = decodeRefName(branchRef);
	const [{ data: branchDetails }, { data: branchDiff }] = useSuspenseQueries({
		queries: [
			branchDetailsQueryOptions({
				projectId,
				// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
				branchName: decodedBranchRef.replace(/^refs\/heads\//, ""),
				remote: null,
			}),
			branchDiffQueryOptions({ projectId, branch: decodedBranchRef }),
		],
	});

	const selectedChange =
		selectedPath !== undefined
			? branchDiff.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : branchDiff.changes;

	return (
		<div>
			<h3>{branchDetails.name}</h3>
			{branchDetails.prNumber != null && <p>PR #{branchDetails.prNumber}</p>}
			<ChangesFileDiffList
				changes={changes}
				projectId={projectId}
				fileParent={branchFileParent({ stackId, branchRef })}
			/>
		</div>
	);
};

const Details: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef, stackId }) => (
				<BranchDetails projectId={projectId} branchRef={branchRef} stackId={stackId} />
			),
			ChangesSection: () => <ChangesDetails projectId={projectId} />,
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: () => <ChangesDetails projectId={projectId} selectedPath={path} />,
						Branch: ({ branchRef, stackId }) => (
							<BranchDetails
								projectId={projectId}
								branchRef={branchRef}
								selectedPath={path}
								stackId={stackId}
							/>
						),
						Commit: ({ commitId, stackId }) => (
							<CommitDetails
								projectId={projectId}
								commitId={commitId}
								stackId={stackId}
								selectedPath={path}
							/>
						),
					}),
				),
			Commit: ({ commitId, stackId }) => (
				<CommitDetails projectId={projectId} commitId={commitId} stackId={stackId} />
			),
			BaseCommit: () => null,
			Hunk: () => null,
		}),
	);

const EditorHelp: FC<{
	hotkeys: Array<{ hotkey: string; name: string }>;
}> = ({ hotkeys }) => (
	<div className={styles.editorHelp}>
		{hotkeys.map((hotkey, index) => (
			<Fragment key={hotkey.hotkey}>
				{index > 0 && " • "}
				<kbd className={styles.editorShortcut}>{formatForDisplay(hotkey.hotkey)}</kbd> to{" "}
				{hotkey.name}
			</Fragment>
		))}
	</div>
);

type CommandPaletteItem = HotkeyRegistrationView & {
	options: { meta: { group: HotkeyGroup; name: string } };
};

const groupCommandPaletteItems = (
	commands: Array<CommandPaletteItem>,
): Array<PickerDialogGroup<CommandPaletteItem>> => {
	const groups = new Map<string, Array<CommandPaletteItem>>();

	for (const command of commands) {
		const groupName = command.options.meta.group;
		const group = groups.get(groupName);
		if (group) group.push(command);
		else groups.set(groupName, [command]);
	}

	return globalThis.Array.from(groups.entries())
		.sort(([a], [b]) => a.localeCompare(b))
		.map(([value, items]) => ({
			value,
			items: globalThis.Array.from(items).sort((a, b) =>
				a.options.meta.name.localeCompare(b.options.meta.name),
			),
		}));
};

const CommandPalette: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
}> = ({ open, onOpenChange }) => {
	const { hotkeys } = useHotkeyRegistrations();
	const items = pipe(
		hotkeys
			.filter(
				(hotkey): hotkey is CommandPaletteItem =>
					hotkey.options.enabled !== false &&
					hotkey.options.meta?.name !== undefined &&
					hotkey.options.meta.commandPalette !== false,
			)
			.sort((a, b) => a.options.meta.name.localeCompare(b.options.meta.name)),
		groupCommandPaletteItems,
	);

	const runCommand = (hotkey: CommandPaletteItem) => {
		onOpenChange(false);
		getHotkeyManager().triggerRegistration(hotkey.id);
	};

	return (
		<PickerDialog
			ariaLabel="Command palette"
			closeLabel="Close command palette"
			emptyLabel="No commands found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.options.meta.name}
			getItemType={(x) =>
				x.options.meta.commandPalette !== "hideHotkey" ? formatForDisplay(x.hotkey) : undefined
			}
			items={items}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={runCommand}
			placeholder="Search commands…"
		/>
	);
};

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

	useHotkey("Enter", () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Reword commit", name: "Save", commandPalette: false },
	});

	useHotkey("Escape", onExit, {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Reword commit", name: "Cancel", commandPalette: false },
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
		isExpanded: boolean;
		projectId: string;
		stackId: string;
		navigationIndex: NavigationIndex;
		focusPanel: (panel: PanelType) => void;
	} & ComponentProps<"div">
> = ({ commit, isExpanded, projectId, stackId, navigationIndex, focusPanel, ...restProps }) => {
	const isHighlighted = useAppSelector((state) =>
		selectProjectHighlightedCommitIds(state, projectId).includes(commit.id),
	);
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
		operandEquals(
			operand,
			commitOperand({
				stackId: outlineMode.stackId,
				commitId: outlineMode.commitId,
			}),
		);
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);
	const [isCommitMessagePending, startCommitMessageTransition] = useTransition();
	// We use a controlled tooltip as a workaround for https://github.com/mui/base-ui/issues/4499.
	const [isExpandCollapseTooltipOpen, setIsExpandCollapseTooltipOpen] = useState(false);

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};

	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const commitDiscard = useMutation(commitDiscardMutationOptions);
	const commitReword = useMutation(commitRewordMutationOptions);

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

	const cutCommit = () => {
		dispatch(projectActions.enterMoveMode({ projectId, source: operand }));
	};

	const startEditing = () => {
		dispatch(projectActions.startRewordCommit({ projectId, commit: commitOperandV }));
	};
	const focusedPanel = useFocusedProjectPanel(projectId);

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.select({ projectId, selection: operand }));
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

	const menuItems: Array<NativeMenuItem> = [
		{
			_tag: "Item",
			label: "Cut commit",
			onSelect: cutCommit,
		},
		{
			_tag: "Item",
			label: "Reword commit",
			enabled: !isCommitMessagePending,
			onSelect: startEditing,
		},
		{
			_tag: "Item",
			label: "Add empty commit",
			submenu: [
				{
					_tag: "Item",
					label: "Above",
					onSelect: insertBlankCommitAbove,
				},
				{
					_tag: "Item",
					label: "Below",
					onSelect: insertBlankCommitBelow,
				},
			],
		},
		{
			_tag: "Item",
			label: "Delete commit",
			enabled: !commitDiscard.isPending,
			onSelect: deleteCommit,
		},
	];

	useHotkey("Enter", startEditing, {
		conflictBehavior: "allow",
		enabled:
			!isCommitMessagePending &&
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default",
		meta: { group: "Commit", name: "Reword" },
	});

	useHotkey(
		"ArrowRight",
		() => {
			dispatch(projectActions.openCommitFiles({ projectId, commit: commitOperandV }));
		},
		{
			conflictBehavior: "allow",
			enabled:
				isSelected && focusedPanel === "outline" && outlineMode._tag === "Default" && !isExpanded,
			meta: { group: "Commit", name: "Expand files" },
		},
	);

	useHotkey(
		"ArrowLeft",
		() => {
			dispatch(projectActions.closeCommitFiles({ projectId }));
		},
		{
			conflictBehavior: "allow",
			enabled:
				isSelected && focusedPanel === "outline" && outlineMode._tag === "Default" && isExpanded,
			meta: { group: "Commit", name: "Collapse files" },
		},
	);

	useHotkey({ key: "" }, insertBlankCommitAbove, {
		conflictBehavior: "allow",
		enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
		meta: {
			group: "Commit",
			name: "Add empty commit above",
			commandPalette: "hideHotkey",
			shortcutsBar: false,
		},
	});

	useHotkey({ key: "" }, insertBlankCommitBelow, {
		conflictBehavior: "allow",
		enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
		meta: {
			group: "Commit",
			name: "Add empty commit below",
			commandPalette: "hideHotkey",
			shortcutsBar: false,
		},
	});

	useHotkey({ key: "" }, deleteCommit, {
		conflictBehavior: "allow",
		enabled:
			!commitDiscard.isPending &&
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default",
		meta: {
			group: "Commit",
			name: "Delete commit",
			commandPalette: "hideHotkey",
			shortcutsBar: false,
		},
	});

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			navigationIndex={navigationIndex}
			className={classes(restProps.className, isHighlighted && styles.itemRowHighlighted)}
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
						className={styles.itemRowLabel}
						onContextMenu={
							outlineMode._tag === "Default"
								? (event) => {
										void showNativeContextMenu(event, menuItems);
									}
								: undefined
						}
					>
						<CommitLabel commit={commitWithOptimisticMessage} />
					</div>
					{outlineMode._tag === "Default" && (
						<ItemRowToolbar aria-label="Commit actions">
							<Tooltip.Root
								open={isExpandCollapseTooltipOpen}
								// Prevent tooltip from lingering while moving between nearby controls.
								// [tag:tooltip-disable-hoverable-popup]
								disableHoverablePopup
							>
								<Tooltip.Trigger
									render={<Toolbar.Button type="button" className={styles.itemRowToolbarButton} />}
									onClick={() =>
										dispatch(
											projectActions.toggleCommitFiles({ projectId, commit: commitOperandV }),
										)
									}
									onMouseEnter={() => setIsExpandCollapseTooltipOpen(true)}
									onMouseLeave={() => setIsExpandCollapseTooltipOpen(false)}
									onFocus={() => setIsExpandCollapseTooltipOpen(true)}
									onBlur={() => setIsExpandCollapseTooltipOpen(false)}
									aria-label={"Toggle commit files"}
								>
									<ExpandCollapseIcon isExpanded={isExpanded} />
								</Tooltip.Trigger>
								<Tooltip.Portal>
									<Tooltip.Positioner sideOffset={8}>
										<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
											Toggle commit files
										</Tooltip.Popup>
									</Tooltip.Positioner>
								</Tooltip.Portal>
							</Tooltip.Root>
							<Toolbar.Button
								type="button"
								className={styles.itemRowToolbarButton}
								aria-label="Commit menu"
								onClick={(event) => {
									void showNativeMenuFromTrigger(event.currentTarget, menuItems);
								}}
							>
								<MenuTriggerIcon />
							</Toolbar.Button>
						</ItemRowToolbar>
					)}
				</>
			)}
		</ItemRow>
	);
};

const CommitFileRow: FC<{
	change: TreeChange;
	parentCommitOperand: CommitOperand;
	navigationIndex: NavigationIndex;
	projectId: string;
}> = ({ change, parentCommitOperand, navigationIndex, projectId }) => {
	const dispatch = useAppDispatch();
	const operand = fileOperand({
		parent: commitFileParent(parentCommitOperand),
		path: change.path,
	});
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);

	useHotkey(
		"ArrowLeft",
		() => {
			const parentOperand = commitOperand(parentCommitOperand);
			dispatch(projectActions.select({ projectId, selection: parentOperand }));
			focusTreeItem(projectId, parentOperand);
		},
		{
			conflictBehavior: "allow",
			enabled: isSelected && focusedPanel === "outline",
			meta: {
				group: "Commit file",
				name: "Select commit",
				commandPalette: false,
				shortcutsBar: false,
			},
		},
	);

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={fileRowLabel(change)}
			render={
				<OperandC
					projectId={projectId}
					operand={operand}
					render={
						<ItemRow
							projectId={projectId}
							operand={operand}
							navigationIndex={navigationIndex}
							className={styles.fileRow}
						/>
					}
				/>
			}
		>
			<div className={styles.itemRowLabel}>{fileRowLabel(change)}</div>
		</TreeItem>
	);
};

const CommitC: FC<{
	commit: Commit;
	projectId: string;
	stackId: string;
	navigationIndex: NavigationIndex;
	focusPanel: (panel: PanelType) => void;
}> = ({ commit, projectId, stackId, navigationIndex, focusPanel }) => {
	const isExpanded = useAppSelector(
		(state) => selectProjectExpandedCommitId(state, projectId) === commit.id,
	);
	const commitOperandV: CommitOperand = { stackId, commitId: commit.id };
	const operand = commitOperand(commitOperandV);

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={commitTitle(commit.message)}
			expanded={isExpanded}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<CommitRow
				commit={commit}
				isExpanded={isExpanded}
				projectId={projectId}
				stackId={stackId}
				navigationIndex={navigationIndex}
				focusPanel={focusPanel}
			/>
			{isExpanded && (
				<Suspense fallback={<div className={styles.itemRowEmpty}>Loading commit files…</div>}>
					<CommitFiles
						projectId={projectId}
						commitId={commit.id}
						parentCommitOperand={commitOperandV}
						navigationIndex={navigationIndex}
					/>
				</Suspense>
			)}
		</TreeItem>
	);
};

const ChangesFileRow: FC<{
	change: TreeChange;
	dependencyCommitIds: NonEmptyArray<string> | undefined;
	navigationIndex: NavigationIndex;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	projectId: string;
}> = ({ change, dependencyCommitIds, navigationIndex, onAbsorbChanges, projectId }) => {
	const operand = fileOperand({ parent: changesFileParent, path: change.path });
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	useHotkey(
		"A",
		() => {
			onAbsorbChanges({
				type: "treeChanges",
				subject: {
					changes: [change],
					assignedStackId: null,
				},
			});
		},
		{
			conflictBehavior: "allow",
			enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
			meta: { group: "Changes file", name: "Absorb" },
		},
	);

	const menuItems: Array<NativeMenuItem> = [
		{
			_tag: "Item",
			label: "Absorb",
			onSelect: () => {
				onAbsorbChanges({
					type: "treeChanges",
					subject: {
						changes: [change],
						assignedStackId: null,
					},
				});
			},
		},
	];

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={fileRowLabel(change)}
			render={
				<OperandC
					projectId={projectId}
					operand={operand}
					render={
						<ItemRow projectId={projectId} operand={operand} navigationIndex={navigationIndex} />
					}
				/>
			}
		>
			<div
				className={styles.itemRowLabel}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				{fileRowLabel(change)}
			</div>
			{outlineMode._tag === "Default" && (
				<ItemRowToolbar aria-label="File actions">
					{dependencyCommitIds && (
						<DependencyIndicatorButton
							projectId={projectId}
							commitIds={dependencyCommitIds}
							render={<Toolbar.Button type="button" className={styles.itemRowToolbarButton} />}
						>
							<DependencyIcon />
						</DependencyIndicatorButton>
					)}
					<Toolbar.Button
						type="button"
						className={styles.itemRowToolbarButton}
						aria-label="File menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
					>
						<MenuTriggerIcon />
					</Toolbar.Button>
				</ItemRowToolbar>
			)}
		</TreeItem>
	);
};

const ChangesSectionRow: FC<{
	changes: Array<TreeChange>;
	navigationIndex: NavigationIndex;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	onCommit: () => void;
	projectId: string;
}> = ({ changes, navigationIndex, onAbsorbChanges, onCommit, projectId }) => {
	const operand = changesSectionOperand;
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	useHotkey(
		"A",
		() => {
			onAbsorbChanges({ type: "all" });
		},
		{
			conflictBehavior: "allow",
			enabled:
				changes.length > 0 &&
				isSelected &&
				focusedPanel === "outline" &&
				outlineMode._tag === "Default",
			meta: { group: "Changes", name: "Absorb" },
		},
	);

	const menuItems: Array<NativeMenuItem> = [
		{
			_tag: "Item",
			label: "Absorb",
			enabled: changes.length > 0,
			onSelect: () => {
				onAbsorbChanges({ type: "all" });
			},
		},
	];

	return (
		<ItemRow projectId={projectId} operand={operand} navigationIndex={navigationIndex}>
			<div
				className={classes(styles.itemRowLabel, styles.sectionLabel)}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				Changes ({changes.length})
			</div>
			{outlineMode._tag === "Default" && (
				<ItemRowToolbar aria-label="Changes actions">
					<Toolbar.Button type="button" className={styles.itemRowToolbarButton} onClick={onCommit}>
						Commit
					</Toolbar.Button>
					<Toolbar.Button
						type="button"
						className={styles.itemRowToolbarButton}
						aria-label="Changes menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
					>
						<MenuTriggerIcon />
					</Toolbar.Button>
				</ItemRowToolbar>
			)}
		</ItemRow>
	);
};

const BaseCommit: FC<{
	projectId: string;
	commitId?: string;
	navigationIndex: NavigationIndex;
}> = ({ projectId, commitId, navigationIndex }) => {
	const operand = baseCommitOperand;

	return (
		<div className={styles.section}>
			<TreeItem
				projectId={projectId}
				operand={operand}
				label="Base commit"
				render={
					<OperandC
						projectId={projectId}
						operand={operand}
						render={
							<ItemRow projectId={projectId} operand={operand} navigationIndex={navigationIndex} />
						}
					/>
				}
			>
				<div className={classes(styles.itemRowLabel, styles.sectionLabel)}>
					{commitId !== undefined
						? `${shortCommitId(commitId)} (common base commit)`
						: "(base commit)"}
				</div>
			</TreeItem>
		</div>
	);
};

const Changes: FC<{
	projectId: string;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	onCommit: () => void;
	navigationIndex: NavigationIndex;
}> = ({ projectId, onAbsorbChanges, onCommit, navigationIndex }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	const operand = changesSectionOperand;

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={`Changes (${worktreeChanges.changes.length})`}
			expanded
			className={styles.section}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<ChangesSectionRow
				changes={worktreeChanges.changes}
				navigationIndex={navigationIndex}
				onAbsorbChanges={onAbsorbChanges}
				onCommit={onCommit}
				projectId={projectId}
			/>
			{worktreeChanges.changes.length === 0 ? (
				<div className={styles.itemRowEmpty}>No changes.</div>
			) : (
				<div role="group">
					{worktreeChanges.changes.map((change) => {
						const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
						const dependencyCommitIds = hunkDependencyDiffs
							? getDependencyCommitIds({ hunkDependencyDiffs })
							: undefined;

						return (
							<ChangesFileRow
								key={change.path}
								change={change}
								dependencyCommitIds={dependencyCommitIds}
								navigationIndex={navigationIndex}
								onAbsorbChanges={onAbsorbChanges}
								projectId={projectId}
							/>
						);
					})}
				</div>
			)}
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

	useHotkey("Enter", () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Rename branch", name: "Save", commandPalette: false },
	});

	useHotkey("Escape", onExit, {
		conflictBehavior: "allow",
		enabled: focusedPanel === "outline",
		ignoreInputs: false,
		meta: { group: "Rename branch", name: "Cancel", commandPalette: false },
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
		navigationIndex: NavigationIndex;
		focusPanel: (panel: PanelType) => void;
	} & ComponentProps<"div">
> = ({ projectId, branchName, branchRef, stackId, navigationIndex, focusPanel, ...restProps }) => {
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const dispatch = useAppDispatch();
	const branchOperandV: BranchOperand = {
		stackId,
		branchRef,
	};
	const operand = branchOperand(branchOperandV);
	const isRenaming =
		outlineMode._tag === "RenameBranch" &&
		operandEquals(
			operand,
			branchOperand({
				stackId: outlineMode.stackId,
				branchRef: outlineMode.branchRef,
			}),
		);
	const [optimisticBranchName, setOptimisticBranchName] = useOptimistic(
		branchName,
		(_currentBranchName, nextBranchName: string) => nextBranchName,
	);
	const [isRenamePending, startRenameTransition] = useTransition();

	const updateBranchName = useMutation(updateBranchNameMutationOptions);

	const startEditing = () => {
		dispatch(projectActions.startRenameBranch({ projectId, branch: branchOperandV }));
	};
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.select({ projectId, selection: operand }));
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
			const newItem = branchOperand({
				stackId,
				// TODO: ideally the API would return the new ref?
				branchRef: encodeRefName(`refs/heads/${trimmed}`),
			});
			dispatch(projectActions.select({ projectId, selection: newItem }));
			dispatch(projectActions.exitMode({ projectId }));
		});
	};

	const menuItems: Array<NativeMenuItem> = [
		{
			_tag: "Item",
			label: "Rename branch",
			enabled: !isRenamePending,
			onSelect: startEditing,
		},
	];

	useHotkey("Enter", startEditing, {
		conflictBehavior: "allow",
		enabled: isSelected && focusedPanel === "outline" && outlineMode._tag === "Default",
		meta: { group: "Branch", name: "Rename" },
	});

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			navigationIndex={navigationIndex}
		>
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
						className={classes(styles.itemRowLabel, styles.sectionLabel)}
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
						<ItemRowToolbar aria-label="Branch actions">
							<Toolbar.Button
								type="button"
								className={styles.itemRowToolbarButton}
								aria-label="Push branch"
								disabled
							>
								<PushIcon />
							</Toolbar.Button>
							<Toolbar.Button
								type="button"
								className={styles.itemRowToolbarButton}
								aria-label="Branch menu"
								onClick={(event) => {
									void showNativeMenuFromTrigger(event.currentTarget, menuItems);
								}}
							>
								<MenuTriggerIcon />
							</Toolbar.Button>
						</ItemRowToolbar>
					)}
				</>
			)}
		</ItemRow>
	);
};

const StackRow: FC<
	{
		navigationIndex: NavigationIndex;
		projectId: string;
		stackId: string;
	} & ComponentProps<"div">
> = ({ navigationIndex, projectId, stackId, ...restProps }) => {
	const operand = stackOperand({ stackId });
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const unapplyStack = useMutation(unapplyStackMutationOptions);
	const unapply = () => {
		unapplyStack.mutate({ projectId, stackId });
	};

	const menuItems: Array<NativeMenuItem> = [
		{ _tag: "Item", label: "Move up", enabled: false },
		{ _tag: "Item", label: "Move down", enabled: false },
		{ _tag: "Separator" },
		{
			_tag: "Item",
			label: "Unapply stack",
			enabled: !unapplyStack.isPending,
			onSelect: unapply,
		},
	];

	useHotkey({ key: "" }, unapply, {
		conflictBehavior: "allow",
		enabled:
			isSelected &&
			focusedPanel === "outline" &&
			outlineMode._tag === "Default" &&
			!unapplyStack.isPending,
		meta: {
			group: "Stack",
			name: "Unapply stack",
			commandPalette: "hideHotkey",
			shortcutsBar: false,
		},
	});

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={operand}
			navigationIndex={navigationIndex}
		>
			<div
				className={classes(styles.itemRowLabel, styles.sectionLabel)}
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
				<ItemRowToolbar aria-label="Stack actions">
					<Toolbar.Button
						type="button"
						className={styles.itemRowToolbarButton}
						aria-label="Stack menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
					>
						<MenuTriggerIcon />
					</Toolbar.Button>
				</ItemRowToolbar>
			)}
		</ItemRow>
	);
};

const BranchSegment: FC<{
	navigationIndex: NavigationIndex;
	projectId: string;
	segment: Segment;
	stackId: string;
	focusPanel: (panel: PanelType) => void;
}> = ({ navigationIndex, projectId, segment, stackId, focusPanel }) => {
	const refName = assert(segment.refName);
	const operand = branchOperand({ stackId, branchRef: refName.fullNameBytes });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={refName.displayName}
			expanded
			className={classes(styles.section, styles.segment)}
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
						navigationIndex={navigationIndex}
						focusPanel={focusPanel}
					/>
				}
			/>

			{segment.commits.length === 0 ? (
				<div className={styles.itemRowEmpty}>No commits.</div>
			) : (
				<div role="group">
					{segment.commits.map((commit) => (
						<CommitC
							key={commit.id}
							commit={commit}
							projectId={projectId}
							stackId={stackId}
							navigationIndex={navigationIndex}
							focusPanel={focusPanel}
						/>
					))}
				</div>
			)}
		</TreeItem>
	);
};

const BranchlessSegment: FC<{
	navigationIndex: NavigationIndex;
	projectId: string;
	segment: Segment;
	stackId: string;
	focusPanel: (panel: PanelType) => void;
}> = ({ navigationIndex, projectId, segment, stackId, focusPanel }) => (
	<div className={classes(styles.section, styles.segment)}>
		{segment.commits.map((commit) => (
			<CommitC
				key={commit.id}
				commit={commit}
				projectId={projectId}
				stackId={stackId}
				navigationIndex={navigationIndex}
				focusPanel={focusPanel}
			/>
		))}
	</div>
);

const StackC: FC<{
	projectId: string;
	stack: Stack;
	navigationIndex: NavigationIndex;
	focusPanel: (panel: PanelType) => void;
}> = ({ projectId, stack, navigationIndex, focusPanel }) => {
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
			className={classes(styles.stack, styles.section)}
			render={<OperandC projectId={projectId} operand={operand} />}
		>
			<StackRow
				projectId={projectId}
				stackId={stackId}
				navigationIndex={navigationIndex}
				className={styles.stackRow}
			/>

			<div role="group" className={styles.segments}>
				{stack.segments.map((segment) => {
					const branchRef = segment.refName?.fullNameBytes;

					if (!branchRef && segment.commits.length === 0) return null;

					const segmentKey = branchRef
						? JSON.stringify(branchRef)
						: // A segment should always either have a branch reference or at
							// least one commit, so this assertion should be safe.
							assert(segment.commits[0]).id;

					return branchRef ? (
						<BranchSegment
							key={segmentKey}
							navigationIndex={navigationIndex}
							projectId={projectId}
							segment={segment}
							stackId={stackId}
							focusPanel={focusPanel}
						/>
					) : (
						<BranchlessSegment
							key={segmentKey}
							navigationIndex={navigationIndex}
							projectId={projectId}
							segment={segment}
							stackId={stackId}
							focusPanel={focusPanel}
						/>
					);
				})}
			</div>
		</TreeItem>
	);
};

type BranchPickerOption = {
	id: string;
	label: string;
	branch: BranchOperand;
};

const segmentToBranchPickerOption = ({
	segment,
	stackId,
}: {
	segment: Segment;
	stackId: string;
}): BranchPickerOption | null => {
	const refName = segment.refName;
	if (!refName) return null;

	return {
		id: JSON.stringify([stackId, refName.fullNameBytes]),
		label: refName.displayName,
		branch: { stackId, branchRef: refName.fullNameBytes },
	};
};

const stackToBranchPickerOptions = (stack: Stack): Array<BranchPickerOption> => {
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
	const stackId = stack.id!;
	return stack.segments.flatMap((segment): Array<BranchPickerOption> => {
		const option = segmentToBranchPickerOption({ segment, stackId });
		return option ? [option] : [];
	});
};

const BranchPicker: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onSelectBranch: (branch: BranchOperand) => void;
	stacks: Array<Stack>;
}> = ({ open, onOpenChange, onSelectBranch, stacks }) => {
	const selectBranch = (option: BranchPickerOption) => {
		onOpenChange(false);
		onSelectBranch(option.branch);
	};

	return (
		<PickerDialog
			ariaLabel="Select branch"
			closeLabel="Close branch picker"
			emptyLabel="No results found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.label}
			getItemType={() => "Branch"}
			itemToStringValue={(x) => x.label}
			items={[
				{
					value: "Branches",
					items: stacks.flatMap(stackToBranchPickerOptions),
				},
			]}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectBranch}
			placeholder="Search for branches…"
		/>
	);
};

type ApplyBranchPickerOption = {
	branchRef: string;
	label: string;
	type: string;
};

const branchListingToApplyBranchPickerOptions = (
	branch: BranchListing,
): Array<ApplyBranchPickerOption> => {
	if (branch.hasLocal)
		return [
			{
				branchRef: `refs/heads/${branch.name}`,
				label: branch.name,
				type: "Local",
			},
		];

	return branch.remotes.map((remote) => ({
		branchRef: `refs/remotes/${remote}/${branch.name}`,
		label: branch.name,
		type: remote,
	}));
};

const ApplyBranchPicker: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
	projectId: string;
}> = ({ open, onOpenChange, projectId }) => {
	const branchesQuery = useQuery(
		listBranchesQueryOptions({ projectId, filter: { local: null, applied: false } }),
	);
	const items = (branchesQuery.data ?? []).flatMap(branchListingToApplyBranchPickerOptions);
	const applyBranch = useMutation(applyBranchMutationOptions);
	const statusLabel =
		items.length === 0
			? branchesQuery.isPending
				? "Loading branches..."
				: branchesQuery.isError
					? "Unable to load branches."
					: undefined
			: undefined;

	const selectBranch = (option: ApplyBranchPickerOption) => {
		onOpenChange(false);
		applyBranch.mutate({ projectId, existingBranch: option.branchRef });
	};

	return (
		<PickerDialog
			ariaLabel="Apply branch"
			closeLabel="Close apply branch picker"
			emptyLabel="No available branches found."
			getItemKey={(x) => x.branchRef}
			getItemLabel={(x) => x.label}
			getItemType={(x) => x.type}
			itemToStringValue={(x) => x.label}
			items={[
				{
					value: "Available branches",
					items: (branchesQuery.data ?? []).flatMap(branchListingToApplyBranchPickerOptions),
				},
			]}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectBranch}
			placeholder="Search for branches to apply…"
			statusLabel={statusLabel}
		/>
	);
};

const TopBarActions: FC = () => {
	const dispatch = useAppDispatch();
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);
	const openApplyBranchPicker = () => {
		dispatch(projectActions.openApplyBranchPicker({ projectId }));
	};
	const toggleDetails = () => {
		if (focusedPanel === "details" && isPanelVisible(panelsState, "details")) {
			const detailsPanelIndex = panelsState.visiblePanels.indexOf("details");
			const nextPanel = panelsState.visiblePanels[detailsPanelIndex - 1];
			if (nextPanel !== undefined)
				document.getElementById(nextPanel)?.focus({ focusVisible: false });
		}

		dispatch(projectActions.togglePanel({ projectId, panel: "details" }));
	};

	const toggleDetailsHotkey = "D";
	const applyBranchHotkey = "Shift+A";

	useHotkey(applyBranchHotkey, openApplyBranchPicker, {
		meta: { group: "Branches", name: "Apply" },
	});

	useHotkey(toggleDetailsHotkey, toggleDetails, {
		meta: { group: "Details", name: isPanelVisible(panelsState, "details") ? "Close" : "Open" },
	});

	return (
		<>
			<ShortcutButton hotkey={applyBranchHotkey} onClick={openApplyBranchPicker}>
				Apply
			</ShortcutButton>
			<ShortcutButton
				hotkey={toggleDetailsHotkey}
				aria-pressed={isPanelVisible(panelsState, "details")}
				onClick={toggleDetails}
			>
				Details
			</ShortcutButton>
		</>
	);
};

const isInputIgnoredHotkey = ({
	activeElement,
	hotkey,
}: {
	activeElement: Element | null;
	hotkey: HotkeyRegistrationView;
}): boolean =>
	hotkey.options.ignoreInputs !== false &&
	isInputElement(activeElement) &&
	activeElement !== hotkey.target;

const ShortcutsBar: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const activeElement = useActiveElement();
	const { hotkeys } = useHotkeyRegistrations();
	const visibleHotkeys = hotkeys.filter(
		(hotkey) =>
			hotkey.options.enabled !== false &&
			!isInputIgnoredHotkey({ activeElement, hotkey }) &&
			hotkey.options.meta?.name !== undefined &&
			hotkey.options.meta.shortcutsBar !== false,
	);

	if (visibleHotkeys.length === 0) return null;

	return (
		<div className={styles.shortcutsBarContainer}>
			<span className={styles.shortcutsBarScope}>{focusedPanel ?? "Shortcuts"}</span>
			{visibleHotkeys.map((hotkey) => (
				<div key={hotkey.id} className={styles.shortcutsBarItem}>
					<kbd className={styles.shortcutsBarKeys}>{formatForDisplay(hotkey.hotkey)}</kbd>
					<span>{hotkey.options.meta?.name}</span>
				</div>
			))}
		</div>
	);
};

export const WorkspacePage: FC = () => {
	const dispatch = useAppDispatch();

	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const expandedCommitId = useAppSelector((state) =>
		selectProjectExpandedCommitId(state, projectId),
	);
	const pickerDialog = useAppSelector((state) => selectProjectPickerDialogState(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const { focusAdjacentPanel, focusPanel, panelElementRef } = useProjectPanelFocusManager();
	const focusedPanel = useFocusedProjectPanel(projectId);

	const workspaceOutline = useWorkspaceOutline({ projectId, expandedCommitId });

	const navigationIndexUnfiltered = buildNavigationIndex(workspaceOutline);

	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		if (
			!isValidOutlineMode({
				mode: outlineMode,
				navigationIndex: navigationIndexUnfiltered,
			})
		)
			dispatch(projectActions.exitMode({ projectId }));
	}, [outlineMode, navigationIndexUnfiltered, projectId, dispatch]);

	const selection = useAppSelector((state) => selectProjectSelection(state, projectId));

	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		if (!navigationIndexIncludes(navigationIndexUnfiltered, selection))
			dispatch(
				projectActions.select({
					projectId,
					selection: changesSectionOperand,
				}),
			);
	}, [navigationIndexUnfiltered, selection, projectId, dispatch]);

	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);

	const navigationIndex =
		outlineMode._tag !== "Default"
			? filterNavigationIndex(
					navigationIndexUnfiltered,
					(operand) =>
						// When entering operation mode, the selection must still be
						// selectable otherwise the details panel will suddenly appear to
						// change and the user may lose sight of their source operand (e.g.
						// hunk).
						operandEquals(selection, operand) ||
						// After selection moves, allow returning selection to the source operand.
						(operationMode?.source && operandEquals(operationMode.source, operand)) ||
						includeOperandForOutlineMode({ mode: outlineMode, operand }),
				)
			: navigationIndexUnfiltered;

	const [absorptionTarget, setAbsorptionTarget] = useState<AbsorptionTarget | null>(null);

	const queryClient = useQueryClient();
	const openAbsorptionDialog = (target: AbsorptionTarget) => {
		// Before opening the dialog, warm cache to avoid showing loading states in
		// the dialog itself. This also ensures we don't show a stale absorption
		// plan whilst the dialog revalidates.
		void queryClient.prefetchQuery(absorptionPlanQueryOptions({ projectId, target })).then(() => {
			setAbsorptionTarget(target);
		});
	};

	useHotkey(
		"Mod+K",
		() => {
			if (pickerDialog._tag === "CommandPalette")
				dispatch(projectActions.closePickerDialog({ projectId }));
			else dispatch(projectActions.openCommandPalette({ projectId, focusedPanel }));
		},
		{
			conflictBehavior: "allow",
			meta: { group: "Global", name: "Command palette", commandPalette: false },
		},
	);

	useHotkey(
		"H",
		() => {
			focusAdjacentPanel(-1);
		},
		{
			enabled: focusedPanel !== null,
			meta: { group: "Panels", name: "Focus previous panel", commandPalette: false },
		},
	);

	useHotkey(
		"L",
		() => {
			focusAdjacentPanel(1);
		},
		{
			enabled: focusedPanel !== null,
			meta: { group: "Panels", name: "Focus next panel", commandPalette: false },
		},
	);

	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: panelsState.visiblePanels,
	});

	const selectAndFocus = (newItem: Operand) => {
		dispatch(projectActions.select({ projectId, selection: newItem }));
		focusTreeItem(projectId, newItem);
	};

	const outlinePanelElementRef = useMergedRefs(panelElementRef("outline"), () => {
		selectAndFocus(changesSectionOperand);
	});

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = projects.find((project) => project.id === projectId);
	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	const selectBranch = (branch: BranchOperand) => {
		dispatch(
			projectActions.select({
				projectId,
				selection: branchOperand(branch),
			}),
		);
		focusPanel("outline");
	};

	const setBranchPickerOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openBranchPicker({ projectId }));
		else dispatch(projectActions.closePickerDialog({ projectId }));
	};

	const setApplyBranchPickerOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openApplyBranchPicker({ projectId }));
		else dispatch(projectActions.closePickerDialog({ projectId }));
	};

	const setCommandPaletteOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openCommandPalette({ projectId, focusedPanel }));
		else dispatch(projectActions.closePickerDialog({ projectId }));
	};

	return (
		<>
			<TopBarActionsPortal>
				<TopBarActions />
			</TopBarActionsPortal>

			<ShortcutsBarPortal>
				<ShortcutsBar />
			</ShortcutsBarPortal>

			<Group className={styles.page} defaultLayout={defaultLayout} onLayoutChange={onLayoutChanged}>
				<OutlinePanel
					elementRef={outlinePanelElementRef}
					focusPanel={focusPanel}
					navigationIndex={navigationIndex}
					onAbsorbChanges={openAbsorptionDialog}
				/>
				{isPanelVisible(panelsState, "details") && (
					<>
						<Separator className={styles.panelResizeHandle} />
						<DetailsPanel elementRef={panelElementRef("details")} focusPanel={focusPanel} />
					</>
				)}
			</Group>

			{absorptionTarget && (
				<AbsorptionDialog
					projectId={projectId}
					target={absorptionTarget}
					onOpenChange={(open) => {
						if (!open) setAbsorptionTarget(null);
					}}
				/>
			)}

			{Match.value(pickerDialog).pipe(
				Match.tagsExhaustive({
					None: () => null,
					ApplyBranchPicker: () => (
						<ApplyBranchPicker open onOpenChange={setApplyBranchPickerOpen} projectId={projectId} />
					),
					BranchPicker: () => (
						<BranchPicker
							open
							onOpenChange={setBranchPickerOpen}
							onSelectBranch={selectBranch}
							stacks={headInfo.stacks}
						/>
					),
					CommandPalette: () => <CommandPalette open onOpenChange={setCommandPaletteOpen} />,
				}),
			)}
		</>
	);
};
