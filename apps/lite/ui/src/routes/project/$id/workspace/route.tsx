import { PatchDiff } from "@pierre/diffs/react";
import {
	commitDiscardMutationOptions,
	commitInsertBlankMutationOptions,
	commitRewordMutationOptions,
	updateBranchNameMutationOptions,
	unapplyStackMutationOptions,
} from "#ui/api/mutations.ts";
import {
	absorptionPlanQueryOptions,
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import { DependencyIcon, ExpandCollapseIcon, MenuTriggerIcon, PushIcon } from "#ui/icons.tsx";
import {
	branchFileParent,
	changesFileParent,
	commitFileParent,
	type FileParent,
} from "#ui/domain/FileParent.ts";
import { getBranchNameByCommitId, getCommonBaseCommitId } from "#ui/domain/RefInfo.ts";
import {
	ProjectPreviewLayout,
	useFocusedProjectPanel,
	useProjectPanelFocusManager,
} from "#ui/routes/project/$id/ProjectPreviewLayout.tsx";
import {
	projectActions,
	selectProjectExpandedCommitId,
	selectProjectHighlightedCommitIds,
	selectProjectOperationModeState,
	selectProjectSelectedItem,
	selectProjectWorkspaceModeState,
} from "#ui/routes/project/$id/state/projectSlice.ts";
import { AbsorptionDialog } from "#ui/routes/project/$id/workspace/Absorption.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { OperationTarget } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { OperationSourceLabel } from "#ui/routes/project/$id/workspace/OperationSourceLabel.tsx";
import {
	formatHunkHeader,
	CommitLabel,
	shortCommitId,
	decodeRefName,
	encodeRefName,
	assert,
	commitTitle,
} from "#ui/routes/project/$id/shared.tsx";
import {
	type NativeMenuItem,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
} from "#ui/native-menu.ts";
import uiStyles from "#ui/ui.module.css";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import {
	AbsorptionTarget,
	Commit,
	DiffHunk,
	HunkDependencies,
	HunkHeader,
	Segment,
	Stack,
	TreeChange,
	UnifiedPatch,
} from "@gitbutler/but-sdk";
import {
	useMutation,
	useQueryClient,
	useSuspenseQueries,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { createRoute } from "@tanstack/react-router";
import { Array, Match, pipe } from "effect";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";
import {
	ComponentProps,
	FC,
	Fragment,
	ReactNode,
	Ref,
	Suspense,
	useEffect,
	useLayoutEffect,
	useOptimistic,
	useRef,
	useState,
	useTransition,
} from "react";
import { Route as projectRoute } from "#ui/routes/project/$id/route.tsx";
import { useAppDispatch, useAppSelector } from "#ui/state/hooks.ts";
import {
	branchItem,
	baseCommitItem,
	changesSectionItem,
	type BranchItem,
	type CommitItem,
	commitItem,
	fileItem,
	itemEquals,
	itemIdentityKey,
	type Item,
	stackItem,
	hunkItem,
} from "./Item.ts";
import {
	buildNavigationIndex,
	filterNavigationIndex,
	navigationIndexIncludes,
	type NavigationIndex,
	useWorkspaceOutline,
} from "./WorkspaceModel.ts";
import { CommandPalette } from "./CommandPalette.tsx";
import {
	getScopeBindings,
	getScopeLabel,
	getScope,
	renameBranchBindings,
	rewordCommitBindings,
	useWorkspaceShortcuts,
} from "./WorkspaceShortcuts.ts";
import { PositionedShortcutsBar } from "../ShortcutsBar.tsx";
import { formatShortcutKeys, ShortcutActionBase, type ShortcutBinding } from "#ui/shortcuts.ts";
import styles from "./route.module.css";
import {
	includeItemForWorkspaceMode,
	isValidWorkspaceMode,
	type WorkspaceMode,
} from "./WorkspaceMode.ts";
import { Panel } from "#ui/routes/project/$id/state/layout.ts";

type HunkDependencyDiff = HunkDependencies["diffs"][number];

const useIsItemSelected = ({ projectId, item }: { projectId: string; item: Item }): boolean =>
	useAppSelector((state) => {
		const selectedItem = selectProjectSelectedItem(state, projectId);

		return itemEquals(selectedItem, item);
	});

const treeItemId = (projectId: string, item: Item): string =>
	`project-${encodeURIComponent(projectId)}-treeitem-${encodeURIComponent(itemIdentityKey(item))}`;

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
	renderFile: (change: TreeChange) => ReactNode;
}> = ({ projectId, commitId, renderFile }) => {
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
						<Fragment key={file.path}>{renderFile(file)}</Fragment>
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
> = ({ className, isSelected, ref: refProp, ...props }) => {
	const rowRef = useRef<HTMLDivElement | null>(null);
	const mergedRef = useMergedRefs(rowRef, refProp);

	useLayoutEffect(() => {
		if (!isSelected) return;
		rowRef.current?.scrollIntoView({
			block: "nearest",
			inline: "nearest",
		});
	}, [isSelected]);

	return (
		<div
			{...props}
			ref={mergedRef}
			className={classes(className, styles.itemRow, isSelected && styles.itemRowSelected)}
		/>
	);
};

const ItemRow: FC<
	{
		projectId: string;
		item: Item;
		navigationIndex: NavigationIndex;
	} & Omit<ComponentProps<typeof ItemRowPresentational>, "inert" | "isSelected">
> = ({ projectId, item, navigationIndex, onClick, ...props }) => {
	const dispatch = useAppDispatch();
	const isSelected = useIsItemSelected({ projectId, item });

	return (
		<ItemRowPresentational
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, item)}
			isSelected={isSelected}
			onClick={(event) => {
				onClick?.(event);
				if (!event.defaultPrevented) dispatch(projectActions.selectItem({ projectId, item }));
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
		item: Item;
		label: string;
		expanded?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, item, label, expanded, render, ...props }) => {
	const isSelected = useIsItemSelected({ projectId, item });

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(projectId, item),
			role: "treeitem",
			"aria-label": label,
			"aria-selected": isSelected,
			"aria-expanded": expanded,
		}),
	});
};

const OperationItem: FC<
	{
		projectId: string;
		item: Item;
	} & useRender.ComponentProps<"div">
> = ({ projectId, item, render, ...props }) => {
	const isSelected = useIsItemSelected({ projectId, item });

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				source={item}
				render={
					<OperationTarget
						projectId={projectId}
						item={item}
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

	const item = hunkItem({
		parent: fileParent,
		path: change.path,
		hunkHeader: hunk,
		isResultOfBinaryToTextConversion,
	});

	return (
		<div>
			<OperationSourceC projectId={projectId} source={item}>
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
				const source = fileItem({ parent: fileParent, path: change.path });

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

const ChangesShow: FC<{
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

const CommitShow: FC<{
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

const BranchShow: FC<{
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

const Show: FC<{
	projectId: string;
	selectedItem: Item;
}> = ({ projectId, selectedItem }) =>
	Match.value(selectedItem).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef, stackId }) => (
				<BranchShow projectId={projectId} branchRef={branchRef} stackId={stackId} />
			),
			ChangesSection: () => <ChangesShow projectId={projectId} />,
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: () => <ChangesShow projectId={projectId} selectedPath={path} />,
						Branch: ({ branchRef, stackId }) => (
							<BranchShow
								projectId={projectId}
								branchRef={branchRef}
								selectedPath={path}
								stackId={stackId}
							/>
						),
						Commit: ({ commitId, stackId }) => (
							<CommitShow
								projectId={projectId}
								commitId={commitId}
								stackId={stackId}
								selectedPath={path}
							/>
						),
					}),
				),
			Commit: ({ commitId, stackId }) => (
				<CommitShow projectId={projectId} commitId={commitId} stackId={stackId} />
			),
			BaseCommit: () => null,
			Hunk: () => null,
		}),
	);

const EditorHelp: FC<{
	bindings: Array<ShortcutBinding<ShortcutActionBase>>;
}> = ({ bindings }) => (
	<div className={styles.editorHelp}>
		{bindings.map((binding, index) => (
			<Fragment key={binding.id}>
				{index > 0 && " • "}
				<span className={styles.editorShortcut}>{formatShortcutKeys(binding.keys)}</span> to{" "}
				{binding.description}
			</Fragment>
		))}
	</div>
);

const InlineRewordCommit: FC<{
	message: string;
	onSubmit: (value: string) => void;
	onExit: () => void;
	formRef?: Ref<HTMLFormElement>;
}> = ({ message, onSubmit, onExit, formRef }) => {
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get("message") as string);
	};
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
			<EditorHelp bindings={rewordCommitBindings} />
		</form>
	);
};

const CommitRow: FC<
	{
		commit: Commit;
		inlineRewordCommitFormRef: Ref<HTMLFormElement>;
		workspaceMode: WorkspaceMode;
		isExpanded: boolean;
		projectId: string;
		stackId: string;
		navigationIndex: NavigationIndex;
		focusPanel: (panel: Panel) => void;
	} & ComponentProps<"div">
> = ({
	commit,
	inlineRewordCommitFormRef,
	workspaceMode,
	isExpanded,
	projectId,
	stackId,
	navigationIndex,
	focusPanel,
	...restProps
}) => {
	const isHighlighted = useAppSelector((state) =>
		selectProjectHighlightedCommitIds(state, projectId).includes(commit.id),
	);

	const dispatch = useAppDispatch();
	const commitItemV: CommitItem = {
		stackId,
		commitId: commit.id,
	};
	const item = commitItem(commitItemV);
	const isSelected = useIsItemSelected({ projectId, item });
	const isRewording =
		isSelected &&
		workspaceMode._tag === "RewordCommit" &&
		itemEquals(
			item,
			commitItem({
				stackId: workspaceMode.stackId,
				commitId: workspaceMode.commitId,
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

	const startEditing = () => {
		dispatch(projectActions.startRewordCommit({ projectId, item: commitItemV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectItem({ projectId, item }));
		focusPanel("primary");
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
					onSelect: () => {
						commitInsertBlank.mutate({
							projectId,
							relativeTo: { type: "commit", subject: commit.id },
							side: "above",
							dryRun: false,
						});
					},
				},
				{
					_tag: "Item",
					label: "Below",
					onSelect: () => {
						commitInsertBlank.mutate({
							projectId,
							relativeTo: { type: "commit", subject: commit.id },
							side: "below",
							dryRun: false,
						});
					},
				},
			],
		},
		{
			_tag: "Item",
			label: "Delete commit",
			enabled: !commitDiscard.isPending,
			onSelect: () => {
				commitDiscard.mutate({
					projectId,
					subjectCommitId: commit.id,
					dryRun: false,
				});
			},
		},
	];

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			item={item}
			navigationIndex={navigationIndex}
			className={classes(restProps.className, isHighlighted && styles.itemRowHighlighted)}
		>
			{isRewording ? (
				<InlineRewordCommit
					formRef={inlineRewordCommitFormRef}
					message={optimisticMessage}
					onSubmit={saveNewMessage}
					onExit={endEditing}
				/>
			) : (
				<>
					<div
						className={styles.itemRowLabel}
						onContextMenu={
							workspaceMode._tag === "Default"
								? (event) => {
										void showNativeContextMenu(event, menuItems);
									}
								: undefined
						}
					>
						<CommitLabel commit={commitWithOptimisticMessage} />
					</div>
					{workspaceMode._tag === "Default" && (
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
										dispatch(projectActions.toggleCommitFiles({ projectId, item: commitItemV }))
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
	parentCommitItem: CommitItem;
	navigationIndex: NavigationIndex;
	projectId: string;
}> = ({ change, parentCommitItem, navigationIndex, projectId }) => {
	const item = fileItem({
		parent: commitFileParent(parentCommitItem),
		path: change.path,
	});

	return (
		<TreeItem
			projectId={projectId}
			item={item}
			label={fileRowLabel(change)}
			render={
				<OperationItem
					projectId={projectId}
					item={item}
					render={
						<ItemRow
							projectId={projectId}
							item={item}
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
	inlineRewordCommitFormRef: Ref<HTMLFormElement>;
	workspaceMode: WorkspaceMode;
	projectId: string;
	stackId: string;
	navigationIndex: NavigationIndex;
	focusPanel: (panel: Panel) => void;
}> = ({
	commit,
	inlineRewordCommitFormRef,
	workspaceMode,
	projectId,
	stackId,
	navigationIndex,
	focusPanel,
}) => {
	const isExpanded = useAppSelector(
		(state) => selectProjectExpandedCommitId(state, projectId) === commit.id,
	);
	const commitItemV: CommitItem = { stackId, commitId: commit.id };
	const item = commitItem(commitItemV);

	return (
		<TreeItem
			projectId={projectId}
			item={item}
			label={commitTitle(commit.message)}
			expanded={isExpanded}
			render={<OperationItem projectId={projectId} item={item} />}
		>
			<CommitRow
				commit={commit}
				inlineRewordCommitFormRef={inlineRewordCommitFormRef}
				workspaceMode={workspaceMode}
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
						renderFile={(change) => (
							<CommitFileRow
								change={change}
								parentCommitItem={commitItemV}
								navigationIndex={navigationIndex}
								projectId={projectId}
							/>
						)}
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
	workspaceMode: WorkspaceMode;
	projectId: string;
}> = ({
	change,
	dependencyCommitIds,
	navigationIndex,
	onAbsorbChanges,
	workspaceMode,
	projectId,
}) => {
	const item = fileItem({ parent: changesFileParent, path: change.path });

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
			item={item}
			label={fileRowLabel(change)}
			render={
				<OperationItem
					projectId={projectId}
					item={item}
					render={<ItemRow projectId={projectId} item={item} navigationIndex={navigationIndex} />}
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
			{workspaceMode._tag === "Default" && (
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
	workspaceMode: WorkspaceMode;
}> = ({ changes, navigationIndex, onAbsorbChanges, onCommit, projectId, workspaceMode }) => {
	const item = changesSectionItem;

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
		<ItemRow projectId={projectId} item={item} navigationIndex={navigationIndex}>
			<div
				className={classes(styles.itemRowLabel, styles.sectionLabel)}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				Changes ({changes.length})
			</div>
			{workspaceMode._tag === "Default" && (
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
	const item = baseCommitItem;

	return (
		<div className={styles.section}>
			<TreeItem
				projectId={projectId}
				item={item}
				label="Base commit"
				render={
					<OperationItem
						projectId={projectId}
						item={item}
						render={<ItemRow projectId={projectId} item={item} navigationIndex={navigationIndex} />}
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
	workspaceMode: WorkspaceMode;
}> = ({ projectId, onAbsorbChanges, onCommit, navigationIndex, workspaceMode }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	const item = changesSectionItem;

	return (
		<TreeItem
			projectId={projectId}
			item={item}
			label={`Changes (${worktreeChanges.changes.length})`}
			expanded
			className={styles.section}
			render={<OperationItem projectId={projectId} item={item} />}
		>
			<ChangesSectionRow
				changes={worktreeChanges.changes}
				navigationIndex={navigationIndex}
				onAbsorbChanges={onAbsorbChanges}
				onCommit={onCommit}
				projectId={projectId}
				workspaceMode={workspaceMode}
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
								workspaceMode={workspaceMode}
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
	formRef?: Ref<HTMLFormElement>;
}> = ({ branchName, onSubmit, onExit, formRef }) => {
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get("branchName") as string);
	};
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
			<EditorHelp bindings={renameBranchBindings} />
		</form>
	);
};

const BranchRow: FC<
	{
		inlineRenameBranchFormRef: Ref<HTMLFormElement>;
		workspaceMode: WorkspaceMode;
		projectId: string;
		branchName: string;
		branchRef: Array<number>;
		stackId: string;
		navigationIndex: NavigationIndex;
		focusPanel: (panel: Panel) => void;
	} & ComponentProps<"div">
> = ({
	inlineRenameBranchFormRef,
	workspaceMode,
	projectId,
	branchName,
	branchRef,
	stackId,
	navigationIndex,
	focusPanel,
	...restProps
}) => {
	const dispatch = useAppDispatch();
	const branchItemV: BranchItem = {
		stackId,
		branchRef,
	};
	const item = branchItem(branchItemV);
	const isRenaming =
		workspaceMode._tag === "RenameBranch" &&
		itemEquals(
			item,
			branchItem({
				stackId: workspaceMode.stackId,
				branchRef: workspaceMode.branchRef,
			}),
		);
	const [optimisticBranchName, setOptimisticBranchName] = useOptimistic(
		branchName,
		(_currentBranchName, nextBranchName: string) => nextBranchName,
	);
	const [isRenamePending, startRenameTransition] = useTransition();

	const updateBranchName = useMutation(updateBranchNameMutationOptions);

	const startEditing = () => {
		dispatch(projectActions.startRenameBranch({ projectId, item: branchItemV }));
	};

	const endEditing = () => {
		dispatch(projectActions.exitMode({ projectId }));
		dispatch(projectActions.selectItem({ projectId, item }));
		focusPanel("primary");
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
			const newItem = branchItem({
				stackId,
				// TODO: ideally the API would return the new ref?
				branchRef: encodeRefName(`refs/heads/${trimmed}`),
			});
			dispatch(projectActions.selectItem({ projectId, item: newItem }));
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

	return (
		<ItemRow {...restProps} projectId={projectId} item={item} navigationIndex={navigationIndex}>
			{isRenaming ? (
				<InlineRenameBranch
					branchName={optimisticBranchName}
					formRef={inlineRenameBranchFormRef}
					onSubmit={saveBranchName}
					onExit={endEditing}
				/>
			) : (
				<>
					<div
						className={classes(styles.itemRowLabel, styles.sectionLabel)}
						onContextMenu={
							workspaceMode._tag === "Default"
								? (event) => {
										void showNativeContextMenu(event, menuItems);
									}
								: undefined
						}
					>
						{optimisticBranchName}
					</div>
					{workspaceMode._tag === "Default" && (
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
		workspaceMode: WorkspaceMode;
	} & ComponentProps<"div">
> = ({ navigationIndex, projectId, stackId, workspaceMode, ...restProps }) => {
	const item = stackItem({ stackId });

	const unapplyStack = useMutation(unapplyStackMutationOptions);

	const menuItems: Array<NativeMenuItem> = [
		{ _tag: "Item", label: "Move up", enabled: false },
		{ _tag: "Item", label: "Move down", enabled: false },
		{ _tag: "Separator" },
		{
			_tag: "Item",
			label: "Unapply stack",
			enabled: !unapplyStack.isPending,
			onSelect: () => {
				unapplyStack.mutate({ projectId, stackId });
			},
		},
	];

	return (
		<ItemRow {...restProps} projectId={projectId} item={item} navigationIndex={navigationIndex}>
			<div
				className={classes(styles.itemRowLabel, styles.sectionLabel)}
				onContextMenu={
					workspaceMode._tag === "Default"
						? (event) => {
								void showNativeContextMenu(event, menuItems);
							}
						: undefined
				}
			>
				Stack
			</div>
			{workspaceMode._tag === "Default" && (
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
	inlineRenameBranchFormRef: Ref<HTMLFormElement>;
	inlineRewordCommitFormRef: Ref<HTMLFormElement>;
	navigationIndex: NavigationIndex;
	projectId: string;
	segment: Segment;
	stackId: string;
	workspaceMode: WorkspaceMode;
	focusPanel: (panel: Panel) => void;
}> = ({
	inlineRenameBranchFormRef,
	inlineRewordCommitFormRef,
	navigationIndex,
	projectId,
	segment,
	stackId,
	workspaceMode,
	focusPanel,
}) => {
	const refName = assert(segment.refName);
	const item = branchItem({ stackId, branchRef: refName.fullNameBytes });

	return (
		<TreeItem
			projectId={projectId}
			item={item}
			label={refName.displayName}
			expanded
			className={classes(styles.section, styles.segment)}
		>
			<OperationItem
				projectId={projectId}
				item={item}
				render={
					<BranchRow
						inlineRenameBranchFormRef={inlineRenameBranchFormRef}
						workspaceMode={workspaceMode}
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
							inlineRewordCommitFormRef={inlineRewordCommitFormRef}
							workspaceMode={workspaceMode}
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
	inlineRewordCommitFormRef: Ref<HTMLFormElement>;
	navigationIndex: NavigationIndex;
	projectId: string;
	segment: Segment;
	stackId: string;
	workspaceMode: WorkspaceMode;
	focusPanel: (panel: Panel) => void;
}> = ({
	inlineRewordCommitFormRef,
	navigationIndex,
	projectId,
	segment,
	stackId,
	workspaceMode,
	focusPanel,
}) => (
	<div className={classes(styles.section, styles.segment)}>
		{segment.commits.map((commit) => (
			<CommitC
				key={commit.id}
				commit={commit}
				inlineRewordCommitFormRef={inlineRewordCommitFormRef}
				workspaceMode={workspaceMode}
				projectId={projectId}
				stackId={stackId}
				navigationIndex={navigationIndex}
				focusPanel={focusPanel}
			/>
		))}
	</div>
);

const StackC: FC<{
	inlineRenameBranchFormRef: Ref<HTMLFormElement>;
	inlineRewordCommitFormRef: Ref<HTMLFormElement>;
	projectId: string;
	stack: Stack;
	workspaceMode: WorkspaceMode;
	navigationIndex: NavigationIndex;
	focusPanel: (panel: Panel) => void;
}> = ({
	inlineRenameBranchFormRef,
	inlineRewordCommitFormRef,
	projectId,
	stack,
	workspaceMode,
	navigationIndex,
	focusPanel,
}) => {
	// From Caleb:
	// > There shouldn't be a way within GitButler to end up with a stack without a
	//   StackId. Users can disrupt our matching against our metadata by playing
	//   with references, but we currently also try to patch it up at certain points
	//   so it probably isn't too common.
	// For now we'll treat this as non-nullable until we identify cases where it
	// could genuinely be null (assuming backend correctness).
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [tag:stack-id-required]
	const stackId = stack.id!;
	const item = stackItem({ stackId });

	return (
		<TreeItem
			projectId={projectId}
			item={item}
			label="Stack"
			expanded
			className={classes(styles.stack, styles.section)}
			render={<OperationItem projectId={projectId} item={item} />}
		>
			<StackRow
				workspaceMode={workspaceMode}
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
							inlineRenameBranchFormRef={inlineRenameBranchFormRef}
							inlineRewordCommitFormRef={inlineRewordCommitFormRef}
							navigationIndex={navigationIndex}
							projectId={projectId}
							segment={segment}
							stackId={stackId}
							workspaceMode={workspaceMode}
							focusPanel={focusPanel}
						/>
					) : (
						<BranchlessSegment
							key={segmentKey}
							inlineRewordCommitFormRef={inlineRewordCommitFormRef}
							navigationIndex={navigationIndex}
							projectId={projectId}
							segment={segment}
							stackId={stackId}
							workspaceMode={workspaceMode}
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
	branch: BranchItem;
};

const branchPickerOptionToStringValue = (option: BranchPickerOption): string => option.label;

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

const ProjectPage: FC = () => {
	const dispatch = useAppDispatch();

	const { id: projectId } = Route.useParams();

	const expandedCommitId = useAppSelector((state) =>
		selectProjectExpandedCommitId(state, projectId),
	);
	const workspaceMode = useAppSelector((state) =>
		selectProjectWorkspaceModeState(state, projectId),
	);
	const { focusAdjacentPanel, focusPanel, panelElementRef } = useProjectPanelFocusManager();
	const focusedPanel = useFocusedProjectPanel();

	const workspaceOutline = useWorkspaceOutline({ projectId, expandedCommitId });

	const navigationIndexUnfiltered = buildNavigationIndex(workspaceOutline);

	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		if (
			!isValidWorkspaceMode({
				mode: workspaceMode,
				navigationIndex: navigationIndexUnfiltered,
			})
		)
			dispatch(projectActions.exitMode({ projectId }));
	}, [workspaceMode, navigationIndexUnfiltered, projectId, dispatch]);

	const selectedItem = useAppSelector((state) => selectProjectSelectedItem(state, projectId));

	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		if (!navigationIndexIncludes(navigationIndexUnfiltered, selectedItem))
			dispatch(
				projectActions.selectItem({
					projectId,
					item: changesSectionItem,
				}),
			);
	}, [navigationIndexUnfiltered, selectedItem, projectId, dispatch]);

	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);

	const navigationIndex =
		workspaceMode._tag !== "Default"
			? filterNavigationIndex(
					navigationIndexUnfiltered,
					(item) =>
						// When entering operation mode, the selected item must still be
						// selectable otherwise the preview will suddenly appear to change
						// and the user may lose sight of their source item (e.g. hunk).
						itemEquals(selectedItem, item) ||
						// After selection moves, allow returning selection to the source item.
						(operationMode?.source && itemEquals(operationMode.source, item)) ||
						includeItemForWorkspaceMode({ mode: workspaceMode, item }),
				)
			: navigationIndexUnfiltered;

	const shortcutScope = getScope({ selectedItem, focusedPanel, workspaceMode });

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

	const inlineRenameBranchFormRef = useRef<HTMLFormElement | null>(null);
	const inlineRewordCommitFormRef = useRef<HTMLFormElement | null>(null);
	const [branchPickerOpen, setBranchPickerOpen] = useState(false);

	useWorkspaceShortcuts({
		inlineRenameBranchFormRef,
		inlineRewordCommitFormRef,
		projectId,
		scope: shortcutScope,
		navigationIndex,
		openAbsorptionDialog,
		openBranchPicker: () => setBranchPickerOpen(true),
		focusPanel,
		focusAdjacentPanel,
	});

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = projects.find((project) => project.id === projectId);
	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	const selectBranch = (option: BranchPickerOption) => {
		setBranchPickerOpen(false);
		dispatch(
			projectActions.selectItem({
				projectId,
				item: branchItem(option.branch),
			}),
		);
	};

	const commit = () =>
		dispatch(
			projectActions.enterMoveMode({
				projectId,
				source: changesSectionItem,
			}),
		);

	return (
		<>
			<ProjectPreviewLayout
				projectId={projectId}
				primaryActiveDescendantId={treeItemId(projectId, selectedItem)}
				panelElementRef={panelElementRef}
				show={
					<Suspense fallback={<div>Loading preview (show)…</div>}>
						<Show projectId={projectId} selectedItem={selectedItem} />
					</Suspense>
				}
			>
				<div className={styles.sections}>
					<Changes
						projectId={projectId}
						onAbsorbChanges={openAbsorptionDialog}
						onCommit={commit}
						navigationIndex={navigationIndex}
						workspaceMode={workspaceMode}
					/>

					{headInfo.stacks.map((stack) => (
						<StackC
							key={stack.id}
							inlineRenameBranchFormRef={inlineRenameBranchFormRef}
							inlineRewordCommitFormRef={inlineRewordCommitFormRef}
							projectId={project.id}
							stack={stack}
							workspaceMode={workspaceMode}
							navigationIndex={navigationIndex}
							focusPanel={focusPanel}
						/>
					))}

					<BaseCommit
						projectId={projectId}
						commitId={getCommonBaseCommitId(headInfo)}
						navigationIndex={navigationIndex}
					/>
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
			</ProjectPreviewLayout>

			{shortcutScope && (
				<PositionedShortcutsBar
					label={getScopeLabel(shortcutScope)}
					items={getScopeBindings(shortcutScope)}
				/>
			)}

			{absorptionTarget && (
				<AbsorptionDialog
					projectId={projectId}
					target={absorptionTarget}
					onOpenChange={(open) => {
						if (!open) setAbsorptionTarget(null);
					}}
				/>
			)}

			<CommandPalette
				ariaLabel="Select branch"
				closeLabel="Close branch picker"
				emptyLabel="No results found."
				getItemKey={(branch) => branch.id}
				getItemLabel={(branch) => branch.label}
				getItemType={() => "Branch"}
				itemToStringValue={branchPickerOptionToStringValue}
				items={[
					{
						value: "Branches",
						items: headInfo.stacks.flatMap(stackToBranchPickerOptions),
					},
				]}
				open={branchPickerOpen}
				onOpenChange={setBranchPickerOpen}
				onSelectItem={selectBranch}
				placeholder="Search for branches…"
			/>
		</>
	);
};

export const Route = createRoute({
	getParentRoute: () => projectRoute,
	path: "workspace",
	component: ProjectPage,
});
