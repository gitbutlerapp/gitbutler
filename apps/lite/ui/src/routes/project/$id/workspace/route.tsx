import { PatchDiff } from "@pierre/diffs/react";
import {
	commitDiscardMutationOptions,
	commitInsertBlankMutationOptions,
	commitRewordMutationOptions,
	updateBranchNameMutationOptions,
	unapplyStackMutationOptions,
} from "#ui/api/mutations.ts";
import {
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
import { changeFileParent, commitFileParent, type FileParent } from "#ui/domain/FileParent.ts";
import { getBranchNameByCommitId, getCommonBaseCommitId } from "#ui/domain/RefInfo.ts";
import { ProjectPreviewLayout } from "#ui/routes/project/$id/ProjectPreviewLayout.tsx";
import {
	projectActions,
	selectProjectExpandedCommitId,
	selectProjectHighlightedCommitIds,
	selectProjectLayoutState,
	selectProjectSelectedItem,
	selectProjectWorkspaceModeState,
} from "#ui/routes/project/$id/state/projectSlice.ts";
import { AbsorptionDialog, useAbsorption } from "#ui/routes/project/$id/workspace/Absorption.tsx";
import { useMonitorDraggedItem } from "#ui/routes/project/$id/workspace/OperationDragAndDrop.tsx";
import { isOperationModeSourceOrTarget } from "#ui/routes/project/$id/workspace/OperationMode.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { resolveOperationSource } from "#ui/routes/project/$id/workspace/ResolvedOperationSource.ts";
import { OperationTarget } from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { OperationSourceLabel } from "#ui/routes/project/$id/workspace/OperationSourceLabel.tsx";
import {
	formatHunkHeader,
	CommitLabel,
	shortCommitId,
	decodeRefName,
	encodeRefName,
	assert,
} from "#ui/routes/project/$id/shared.tsx";
import {
	type NativeMenuItem,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
} from "#ui/native-menu.ts";
import uiStyles from "#ui/ui.module.css";
import { Tooltip } from "@base-ui/react";
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
	useLayoutEffect,
	useOptimistic,
	useRef,
	useTransition,
} from "react";
import { Route as projectRoute } from "#ui/routes/project/$id/route.tsx";
import { useAppDispatch, useAppSelector } from "#ui/state/hooks.ts";
import type { RootState } from "#ui/state/store.ts";
import {
	branchItem,
	baseCommitItem,
	changeFileItem,
	changesSectionItem,
	type BranchItem,
	commitFileItem,
	type CommitItem,
	commitItem,
	itemEquals,
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
	defaultWorkspaceMode,
	getOperationMode,
	isValidWorkspaceMode,
	type OperationMode,
	type WorkspaceMode,
} from "./WorkspaceMode.ts";

type HunkDependencyDiff = HunkDependencies["diffs"][number];

const selectNormalizedSelectedItem = (
	state: RootState,
	{
		projectId,
		navigationIndex,
	}: {
		projectId: string;
		navigationIndex: NavigationIndex;
	},
): Item | null => {
	const selectedItem = selectProjectSelectedItem(state, projectId);
	return selectedItem && navigationIndexIncludes(navigationIndex, selectedItem)
		? selectedItem
		: (navigationIndex.items[0] ?? null);
};

const useIsItemSelected = ({
	projectId,
	item,
	navigationIndex,
}: {
	projectId: string;
	item: Item;
	navigationIndex: NavigationIndex;
}): boolean =>
	useAppSelector((state) => {
		const selectedItem = selectNormalizedSelectedItem(state, {
			projectId,
			navigationIndex,
		});

		return selectedItem !== null && itemEquals(selectedItem, item);
	});

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

const FileButton: FC<
	{
		change: TreeChange;
	} & ComponentProps<"button">
> = ({ change, className, ...restProps }) => (
	<button {...restProps} type="button" className={classes(className, styles.itemRowButton)}>
		{Match.value(change.status).pipe(
			Match.when({ type: "Addition" }, () => "A"),
			Match.when({ type: "Deletion" }, () => "D"),
			Match.when({ type: "Modification" }, () => "M"),
			Match.when({ type: "Rename" }, () => "R"),
			Match.exhaustive,
		)}{" "}
		{change.path}
	</button>
);

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
				<ul>
					{data.changes.map((file) => (
						<li key={file.path}>{renderFile(file)}</li>
					))}
				</ul>
			)}
		</>
	);
};

const ItemRow: FC<
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

const DependencyIndicatorButton: FC<{
	projectId: string;
	commitIds: NonEmptyArray<string>;
	className?: string;
	children: ReactNode;
}> = ({ projectId, commitIds, className, children }) => {
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

	return (
		<Tooltip.Root
			// [ref:tooltip-disable-hoverable-popup]
			disableHoverablePopup
		>
			<Tooltip.Trigger
				type="button"
				className={className}
				onMouseEnter={() => {
					dispatch(
						projectActions.setHighlightedCommitIds({
							projectId,
							commitIds,
						}),
					);
				}}
				onMouseLeave={() => {
					dispatch(projectActions.setHighlightedCommitIds({ projectId, commitIds: null }));
				}}
				aria-label={tooltip}
			>
				{children}
			</Tooltip.Trigger>
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
	patch: Extract<UnifiedPatch, { type: "Patch" }>;
	operationMode: OperationMode | null;
	projectId: string;
	fileParent?: FileParent;
	change: TreeChange;
	hunk: DiffHunk;
	editable: boolean;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
}> = ({
	patch,
	operationMode,
	projectId,
	fileParent,
	change,
	hunk,
	editable,
	hunkDependencyDiffs,
}) => {
	const dependencyCommitIds =
		fileParent?._tag === "Change" && hunkDependencyDiffs
			? getDependencyCommitIds({ hunk, hunkDependencyDiffs })
			: undefined;
	const headerRow = (
		<div className={styles.hunkHeaderRow}>
			{dependencyCommitIds && (
				<DependencyIndicatorButton projectId={projectId} commitIds={dependencyCommitIds}>
					<DependencyIcon />
				</DependencyIndicatorButton>
			)}
			<div className={styles.hunkHeader}>{formatHunkHeader(hunk)}</div>
		</div>
	);

	return (
		<div>
			{fileParent && editable
				? (() => {
						const source = hunkItem({
							parent: fileParent,
							path: change.path,
							hunkHeader: hunk,
						});
						return (
							<OperationSourceC
								operationMode={operationMode}
								projectId={projectId}
								source={source}
								canDrag={() => !patch.subject.isResultOfBinaryToTextConversion}
							>
								{headerRow}
							</OperationSourceC>
						);
					})()
				: headerRow}
			<HunkDiff change={change} diff={hunk.diff} />
		</div>
	);
};

const FileDiff: FC<{
	operationMode: OperationMode | null;
	projectId: string;
	change: TreeChange;
	fileParent?: FileParent;
	editable: boolean;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
	diff: UnifiedPatch | null;
}> = ({ operationMode, projectId, change, fileParent, editable, hunkDependencyDiffs, diff }) =>
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
								patch={patch}
								operationMode={operationMode}
								projectId={projectId}
								fileParent={fileParent}
								change={change}
								hunk={hunk}
								editable={editable}
								hunkDependencyDiffs={hunkDependencyDiffs}
							/>
						</li>
					))}
				</ul>
			);
		}),
		Match.exhaustive,
	);

const ChangesPreview: FC<{
	operationMode: OperationMode | null;
	projectId: string;
	selectedPath?: string;
}> = ({ operationMode, projectId, selectedPath }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);
	const selectedChange =
		selectedPath !== undefined
			? worktreeChanges.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : worktreeChanges.changes;
	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(changes, Array.zip(treeChangeDiffs));

	return (
		<div>
			{changesWithDiffs.length === 0 ? (
				<div>No file changes.</div>
			) : (
				<ul>
					{changesWithDiffs.map(([change, diff]) => {
						const source = changeFileItem({ path: change.path });
						return (
							<li key={change.path}>
								<OperationSourceC
									operationMode={operationMode}
									projectId={projectId}
									source={source}
								>
									<h4>{change.path}</h4>
								</OperationSourceC>
								<FileDiff
									operationMode={operationMode}
									projectId={projectId}
									change={change}
									fileParent={changeFileParent}
									editable
									hunkDependencyDiffs={hunkDependencyDiffsByPath.get(change.path)}
									diff={diff}
								/>
							</li>
						);
					})}
				</ul>
			)}
		</div>
	);
};

const CommitPreview: FC<{
	operationMode: OperationMode | null;
	projectId: string;
	commitId: string;
	selectedPath?: string | null;
	editable: boolean;
	stackId: string;
}> = ({ operationMode, projectId, commitId, selectedPath, editable, stackId }) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	const selectedChange =
		selectedPath !== undefined
			? commitDetails.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : commitDetails.changes;
	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(changes, Array.zip(treeChangeDiffs));

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
			{changesWithDiffs.length === 0 ? (
				<div>No file changes.</div>
			) : (
				<ul>
					{changesWithDiffs.map(([change, diff]) => {
						const source = commitFileItem({ stackId, commitId, path: change.path });
						return (
							<li key={change.path}>
								{editable ? (
									<OperationSourceC
										operationMode={operationMode}
										projectId={projectId}
										source={source}
									>
										<h4>{change.path}</h4>
									</OperationSourceC>
								) : (
									<h4>{change.path}</h4>
								)}
								<FileDiff
									operationMode={operationMode}
									projectId={projectId}
									change={change}
									fileParent={commitFileParent({ commitId })}
									editable={editable}
									diff={diff}
								/>
							</li>
						);
					})}
				</ul>
			)}
		</div>
	);
};

const BranchPreview: FC<{
	operationMode: OperationMode | null;
	projectId: string;
	branchRef: Array<number>;
}> = ({ operationMode, projectId, branchRef }) => {
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
	const treeChangeDiffs = useSuspenseQueries({
		queries: branchDiff.changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(branchDiff.changes, Array.zip(treeChangeDiffs));

	return (
		<div>
			<h3>{branchDetails.name}</h3>
			{branchDetails.prNumber != null && <p>PR #{branchDetails.prNumber}</p>}
			{changesWithDiffs.length === 0 ? (
				<div>No file changes.</div>
			) : (
				<ul>
					{changesWithDiffs.map(([change, diff]) => (
						<li key={change.path}>
							<h4>{change.path}</h4>
							<FileDiff
								operationMode={operationMode}
								projectId={projectId}
								change={change}
								editable={false}
								diff={diff}
							/>
						</li>
					))}
				</ul>
			)}
		</div>
	);
};

const Preview: FC<{
	operationMode: OperationMode | null;
	projectId: string;
	selectedItem: Item;
}> = ({ operationMode, projectId, selectedItem }) =>
	Match.value(selectedItem).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef }) => (
				<BranchPreview operationMode={operationMode} projectId={projectId} branchRef={branchRef} />
			),
			ChangesSection: () => <ChangesPreview operationMode={operationMode} projectId={projectId} />,
			ChangeFile: ({ path }) => (
				<ChangesPreview operationMode={operationMode} projectId={projectId} selectedPath={path} />
			),
			Commit: (selectedItem) => (
				<CommitPreview
					operationMode={operationMode}
					projectId={projectId}
					commitId={selectedItem.commitId}
					stackId={selectedItem.stackId}
					editable
				/>
			),
			CommitFile: ({ commitId, path, stackId }) => (
				<CommitPreview
					operationMode={operationMode}
					projectId={projectId}
					commitId={commitId}
					stackId={stackId}
					selectedPath={path}
					editable
				/>
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
	const submit = (event: React.SyntheticEvent<HTMLFormElement>) => {
		event.preventDefault();
		const formData = new FormData(event.currentTarget);
		onExit();
		onSubmit(formData.get("message") as string);
	};
	return (
		<form ref={formRef} className={styles.editorForm} onSubmit={submit} onBlur={submit}>
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
	} & ComponentProps<"div">
> = ({
	commit,
	inlineRewordCommitFormRef,
	workspaceMode,
	isExpanded,
	projectId,
	stackId,
	navigationIndex,
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
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });
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
			inert={!navigationIndexIncludes(navigationIndex, item)}
			isSelected={isSelected}
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
					<button
						type="button"
						className={classes(
							styles.itemRowButton,
							isCommitMessagePending && styles.commitButtonPending,
						)}
						onClick={() => {
							dispatch(projectActions.selectItem({ projectId, item }));
						}}
						onContextMenu={
							workspaceMode._tag === "Default"
								? (event) => {
										void showNativeContextMenu(event, menuItems);
									}
								: undefined
						}
					>
						<CommitLabel commit={commitWithOptimisticMessage} />
					</button>
					{workspaceMode._tag === "Default" && (
						<>
							<Tooltip.Root
								// Prevent tooltip from lingering while moving between nearby controls.
								// [tag:tooltip-disable-hoverable-popup]
								disableHoverablePopup
							>
								<Tooltip.Trigger
									className={styles.itemRowAction}
									type="button"
									onClick={() =>
										dispatch(projectActions.toggleCommitFiles({ projectId, item: commitItemV }))
									}
									aria-expanded={isExpanded}
									aria-label={isExpanded ? "Hide commit files" : "Show commit files"}
								>
									<ExpandCollapseIcon isExpanded={isExpanded} />
								</Tooltip.Trigger>
								<Tooltip.Portal>
									<Tooltip.Positioner sideOffset={8}>
										<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
											{isExpanded ? "Hide commit files" : "Show commit files"}
										</Tooltip.Popup>
									</Tooltip.Positioner>
								</Tooltip.Portal>
							</Tooltip.Root>
							<button
								type="button"
								className={styles.itemRowAction}
								aria-label="Commit menu"
								onClick={(event) => {
									void showNativeMenuFromTrigger(event.currentTarget, menuItems);
								}}
							>
								<MenuTriggerIcon />
							</button>
						</>
					)}
				</>
			)}
		</ItemRow>
	);
};

const CommitFileRow: FC<{
	change: TreeChange;
	operationMode: OperationMode | null;
	parentCommitItem: CommitItem;
	navigationIndex: NavigationIndex;
	projectId: string;
}> = ({ change, operationMode, parentCommitItem, navigationIndex, projectId }) => {
	const dispatch = useAppDispatch();
	const item = commitFileItem({ ...parentCommitItem, path: change.path });
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });

	return (
		<OperationSourceC
			operationMode={operationMode}
			projectId={projectId}
			source={item}
			render={
				<ItemRow
					inert={!navigationIndexIncludes(navigationIndex, item)}
					isSelected={isSelected}
					className={styles.fileRow}
				/>
			}
		>
			<FileButton
				change={change}
				onClick={() => {
					dispatch(projectActions.selectItem({ projectId, item }));
				}}
			/>
		</OperationSourceC>
	);
};

const CommitC: FC<{
	commit: Commit;
	inlineRewordCommitFormRef: Ref<HTMLFormElement>;
	operationMode: OperationMode | null;
	workspaceMode: WorkspaceMode;
	projectId: string;
	stackId: string;
	navigationIndex: NavigationIndex;
}> = ({
	commit,
	inlineRewordCommitFormRef,
	operationMode,
	workspaceMode,
	projectId,
	stackId,
	navigationIndex,
}) => {
	const isExpanded = useAppSelector(
		(state) => selectProjectExpandedCommitId(state, projectId) === commit.id,
	);
	const commitItemV: CommitItem = { stackId, commitId: commit.id };
	const item = commitItem(commitItemV);
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });

	return (
		<OperationSourceC
			operationMode={operationMode}
			projectId={projectId}
			source={item}
			canDrag={() => !isSelected || workspaceMode._tag !== "RewordCommit"}
			render={
				<OperationTarget
					item={item}
					projectId={projectId}
					operationMode={operationMode}
					isSelected={isSelected}
				/>
			}
		>
			<CommitRow
				commit={commit}
				inlineRewordCommitFormRef={inlineRewordCommitFormRef}
				workspaceMode={workspaceMode}
				isExpanded={isExpanded}
				projectId={projectId}
				stackId={stackId}
				navigationIndex={navigationIndex}
			/>
			{isExpanded && (
				<Suspense fallback={<div className={styles.itemRowEmpty}>Loading commit files…</div>}>
					<CommitFiles
						projectId={projectId}
						commitId={commit.id}
						renderFile={(change) => (
							<CommitFileRow
								change={change}
								operationMode={operationMode}
								parentCommitItem={commitItemV}
								navigationIndex={navigationIndex}
								projectId={projectId}
							/>
						)}
					/>
				</Suspense>
			)}
		</OperationSourceC>
	);
};

const ChangeFileRow: FC<{
	change: TreeChange;
	dependencyCommitIds: NonEmptyArray<string> | undefined;
	navigationIndex: NavigationIndex;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	operationMode: OperationMode | null;
	workspaceMode: WorkspaceMode;
	projectId: string;
}> = ({
	change,
	dependencyCommitIds,
	navigationIndex,
	onAbsorbChanges,
	operationMode,
	workspaceMode,
	projectId,
}) => {
	const dispatch = useAppDispatch();
	const item = changeFileItem({ path: change.path });
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });

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
		<OperationSourceC
			operationMode={operationMode}
			projectId={projectId}
			source={item}
			render={
				<ItemRow inert={!navigationIndexIncludes(navigationIndex, item)} isSelected={isSelected} />
			}
		>
			<FileButton
				change={change}
				onClick={() => {
					dispatch(projectActions.selectItem({ projectId, item }));
				}}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			/>
			{workspaceMode._tag === "Default" && (
				<>
					{dependencyCommitIds && (
						<DependencyIndicatorButton
							projectId={projectId}
							commitIds={dependencyCommitIds}
							className={styles.itemRowAction}
						>
							<DependencyIcon />
						</DependencyIndicatorButton>
					)}
					<button
						type="button"
						className={styles.itemRowAction}
						aria-label="File menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
					>
						<MenuTriggerIcon />
					</button>
				</>
			)}
		</OperationSourceC>
	);
};

const ChangesSectionRow: FC<{
	changes: Array<TreeChange>;
	navigationIndex: NavigationIndex;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	projectId: string;
	workspaceMode: WorkspaceMode;
}> = ({ changes, navigationIndex, onAbsorbChanges, projectId, workspaceMode }) => {
	const dispatch = useAppDispatch();
	const item = changesSectionItem;
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });

	const menuItems: Array<NativeMenuItem> = [
		{
			_tag: "Item",
			label: "Absorb",
			enabled: changes.length > 0,
			onSelect: () => {
				onAbsorbChanges({
					type: "treeChanges",
					subject: {
						changes,
						assignedStackId: null,
					},
				});
			},
		},
	];

	return (
		<ItemRow inert={!navigationIndexIncludes(navigationIndex, item)} isSelected={isSelected}>
			<button
				type="button"
				className={classes(styles.itemRowButton, styles.sectionButton)}
				onClick={() => {
					dispatch(projectActions.selectItem({ projectId, item }));
				}}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				Changes
			</button>
			{workspaceMode._tag === "Default" && (
				<button
					type="button"
					className={styles.itemRowAction}
					aria-label="Changes menu"
					onClick={(event) => {
						void showNativeMenuFromTrigger(event.currentTarget, menuItems);
					}}
				>
					<MenuTriggerIcon />
				</button>
			)}
		</ItemRow>
	);
};

const BaseCommitRow: FC<
	{
		projectId: string;
		commitId?: string;
		navigationIndex: NavigationIndex;
		operationMode: OperationMode | null;
	} & ComponentProps<"div">
> = ({ projectId, commitId, navigationIndex, operationMode, ...props }) => {
	const dispatch = useAppDispatch();
	const item = baseCommitItem;
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });

	return (
		<OperationTarget
			projectId={projectId}
			item={item}
			operationMode={operationMode}
			isSelected={isSelected}
			render={
				<ItemRow
					{...props}
					inert={!navigationIndexIncludes(navigationIndex, item)}
					isSelected={isSelected}
				>
					<button
						type="button"
						className={styles.commonBaseCommit}
						onClick={() => {
							dispatch(projectActions.selectItem({ projectId, item }));
						}}
					>
						{commitId !== undefined
							? `${shortCommitId(commitId)} (common base commit)`
							: "(base commit)"}
					</button>
				</ItemRow>
			}
		/>
	);
};

const Changes: FC<{
	operationMode: OperationMode | null;
	projectId: string;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	navigationIndex: NavigationIndex;
	workspaceMode: WorkspaceMode;
}> = ({ operationMode, projectId, onAbsorbChanges, navigationIndex, workspaceMode }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	const item = changesSectionItem;
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });

	return (
		<OperationSourceC
			operationMode={operationMode}
			projectId={projectId}
			source={item}
			className={styles.section}
			render={
				<OperationTarget
					item={item}
					projectId={projectId}
					operationMode={operationMode}
					isSelected={isSelected}
				/>
			}
		>
			<ChangesSectionRow
				changes={worktreeChanges.changes}
				navigationIndex={navigationIndex}
				onAbsorbChanges={onAbsorbChanges}
				projectId={projectId}
				workspaceMode={workspaceMode}
			/>
			{worktreeChanges.changes.length === 0 ? (
				<div className={styles.itemRowEmpty}>No changes.</div>
			) : (
				<ul>
					{worktreeChanges.changes.map((change) => {
						const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
						const dependencyCommitIds = hunkDependencyDiffs
							? getDependencyCommitIds({ hunkDependencyDiffs })
							: undefined;

						return (
							<li key={change.path}>
								<ChangeFileRow
									change={change}
									dependencyCommitIds={dependencyCommitIds}
									navigationIndex={navigationIndex}
									onAbsorbChanges={onAbsorbChanges}
									operationMode={operationMode}
									workspaceMode={workspaceMode}
									projectId={projectId}
								/>
							</li>
						);
					})}
				</ul>
			)}
		</OperationSourceC>
	);
};

const InlineRenameBranch: FC<{
	branchName: string;
	onSubmit: (value: string) => void;
	onExit: () => void;
	formRef?: Ref<HTMLFormElement>;
}> = ({ branchName, onSubmit, onExit, formRef }) => {
	const submit = (event: React.SyntheticEvent<HTMLFormElement>) => {
		event.preventDefault();
		const formData = new FormData(event.currentTarget);
		onExit();
		onSubmit(formData.get("branchName") as string);
	};
	return (
		<form ref={formRef} className={styles.editorForm} onSubmit={submit} onBlur={submit}>
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
		operationMode: OperationMode | null;
		workspaceMode: WorkspaceMode;
		projectId: string;
		branchName: string;
		branchRef: Array<number>;
		stackId: string;
		navigationIndex: NavigationIndex;
	} & ComponentProps<"div">
> = ({
	inlineRenameBranchFormRef,
	operationMode,
	workspaceMode,
	projectId,
	branchName,
	branchRef,
	stackId,
	navigationIndex,
	...restProps
}) => {
	const dispatch = useAppDispatch();
	const branchItemV: BranchItem = {
		stackId,
		branchRef,
	};
	const item = branchItem(branchItemV);
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });
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
		<OperationTarget
			{...restProps}
			projectId={projectId}
			item={item}
			operationMode={operationMode}
			isSelected={isSelected}
			render={
				<OperationSourceC
					operationMode={operationMode}
					projectId={projectId}
					source={item}
					render={
						<ItemRow
							inert={!navigationIndexIncludes(navigationIndex, item)}
							isSelected={isSelected}
						>
							{isRenaming ? (
								<InlineRenameBranch
									branchName={optimisticBranchName}
									formRef={inlineRenameBranchFormRef}
									onSubmit={saveBranchName}
									onExit={endEditing}
								/>
							) : (
								<>
									<button
										type="button"
										className={classes(styles.itemRowButton, styles.sectionButton)}
										onClick={() => dispatch(projectActions.selectItem({ projectId, item }))}
										onContextMenu={
											workspaceMode._tag === "Default"
												? (event) => {
														void showNativeContextMenu(event, menuItems);
													}
												: undefined
										}
									>
										{optimisticBranchName}
									</button>
									{workspaceMode._tag === "Default" && (
										<>
											<button
												type="button"
												className={styles.itemRowAction}
												aria-label="Push branch"
												disabled
											>
												<PushIcon />
											</button>
											<button
												type="button"
												className={styles.itemRowAction}
												aria-label="Branch menu"
												onClick={(event) => {
													void showNativeMenuFromTrigger(event.currentTarget, menuItems);
												}}
											>
												<MenuTriggerIcon />
											</button>
										</>
									)}
								</>
							)}
						</ItemRow>
					}
				/>
			}
		/>
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
	const dispatch = useAppDispatch();
	const item = stackItem({ stackId });
	const isSelected = useIsItemSelected({ projectId, item, navigationIndex });

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
		<ItemRow
			{...restProps}
			isSelected={isSelected}
			inert={!navigationIndexIncludes(navigationIndex, item)}
		>
			<button
				type="button"
				className={classes(styles.itemRowButton, styles.sectionButton)}
				onClick={() => {
					dispatch(projectActions.selectItem({ projectId, item }));
				}}
				onContextMenu={
					workspaceMode._tag === "Default"
						? (event) => {
								void showNativeContextMenu(event, menuItems);
							}
						: undefined
				}
			>
				Stack
			</button>
			{workspaceMode._tag === "Default" && (
				<button
					type="button"
					className={styles.itemRowAction}
					aria-label="Stack menu"
					onClick={(event) => {
						void showNativeMenuFromTrigger(event.currentTarget, menuItems);
					}}
				>
					<MenuTriggerIcon />
				</button>
			)}
		</ItemRow>
	);
};

const SegmentC: FC<{
	inlineRenameBranchFormRef: Ref<HTMLFormElement>;
	inlineRewordCommitFormRef: Ref<HTMLFormElement>;
	navigationIndex: NavigationIndex;
	operationMode: OperationMode | null;
	projectId: string;
	segment: Segment;
	stackId: string;
	workspaceMode: WorkspaceMode;
}> = ({
	inlineRenameBranchFormRef,
	inlineRewordCommitFormRef,
	navigationIndex,
	operationMode,
	projectId,
	segment,
	stackId,
	workspaceMode,
}) => (
	<div className={classes(styles.section, styles.segment)}>
		{segment.refName && (
			<BranchRow
				inlineRenameBranchFormRef={inlineRenameBranchFormRef}
				operationMode={operationMode}
				workspaceMode={workspaceMode}
				projectId={projectId}
				branchName={segment.refName.displayName}
				branchRef={segment.refName.fullNameBytes}
				stackId={stackId}
				navigationIndex={navigationIndex}
			/>
		)}

		{segment.commits.length === 0 ? (
			<div className={styles.itemRowEmpty}>No commits.</div>
		) : (
			<ul>
				{segment.commits.map((commit) => (
					<li key={commit.id}>
						<CommitC
							commit={commit}
							inlineRewordCommitFormRef={inlineRewordCommitFormRef}
							operationMode={operationMode}
							workspaceMode={workspaceMode}
							projectId={projectId}
							stackId={stackId}
							navigationIndex={navigationIndex}
						/>
					</li>
				))}
			</ul>
		)}
	</div>
);

const StackC: FC<{
	inlineRenameBranchFormRef: Ref<HTMLFormElement>;
	inlineRewordCommitFormRef: Ref<HTMLFormElement>;
	operationMode: OperationMode | null;
	projectId: string;
	stack: Stack;
	workspaceMode: WorkspaceMode;
	navigationIndex: NavigationIndex;
}> = ({
	inlineRenameBranchFormRef,
	inlineRewordCommitFormRef,
	operationMode,
	projectId,
	stack,
	workspaceMode,
	navigationIndex,
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

	return (
		<div className={classes(styles.stack, styles.section)}>
			<StackRow
				workspaceMode={workspaceMode}
				projectId={projectId}
				stackId={stackId}
				navigationIndex={navigationIndex}
				className={styles.stackRow}
			/>

			<ul className={styles.segments}>
				{stack.segments.map((segment) => {
					const branchRef = segment.refName?.fullNameBytes;

					if (!branchRef && segment.commits.length === 0) return null;

					const segmentKey = branchRef
						? JSON.stringify(branchRef)
						: // A segment should always either have a branch reference or at
							// least one commit, so this assertion should be safe.
							assert(segment.commits[0]).id;

					return (
						<li key={segmentKey}>
							<SegmentC
								inlineRenameBranchFormRef={inlineRenameBranchFormRef}
								inlineRewordCommitFormRef={inlineRewordCommitFormRef}
								navigationIndex={navigationIndex}
								operationMode={operationMode}
								projectId={projectId}
								segment={segment}
								stackId={stackId}
								workspaceMode={workspaceMode}
							/>
						</li>
					);
				})}
			</ul>
		</div>
	);
};

const ProjectPage: FC = () => {
	const { id: projectId } = Route.useParams();

	const expandedCommitId = useAppSelector((state) =>
		selectProjectExpandedCommitId(state, projectId),
	);
	const layoutState = useAppSelector((state) => selectProjectLayoutState(state, projectId));
	const workspaceModeState = useAppSelector((state) =>
		selectProjectWorkspaceModeState(state, projectId),
	);

	const inlineRenameBranchFormRef = useRef<HTMLFormElement | null>(null);
	const inlineRewordCommitFormRef = useRef<HTMLFormElement | null>(null);

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);

	const project = projects.find((project) => project.id === projectId);

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));

	const workspaceOutline = useWorkspaceOutline({ projectId, expandedCommitId });

	const navigationIndexUnfiltered = buildNavigationIndex(workspaceOutline);

	const queryClient = useQueryClient();

	const workspaceMode = isValidWorkspaceMode({
		mode: workspaceModeState,
		navigationIndex: navigationIndexUnfiltered,
	})
		? workspaceModeState
		: defaultWorkspaceMode;

	const operationMode = getOperationMode(workspaceMode);

	const source = operationMode
		? resolveOperationSource({
				operationSource: operationMode.source,
				queryClient,
				projectId,
			})
		: null;

	const navigationIndex = operationMode
		? filterNavigationIndex(navigationIndexUnfiltered, (item) =>
				isOperationModeSourceOrTarget({
					item,
					operationMode,
					source,
				}),
			)
		: navigationIndexUnfiltered;

	const selectedItem = useAppSelector((state) =>
		selectNormalizedSelectedItem(state, { projectId, navigationIndex }),
	);

	const shortcutScope = getScope({ selectedItem, layoutState, workspaceMode });

	const {
		absorptionPlan,
		isAbsorbing,
		requestAbsorptionPlan,
		confirmAbsorption,
		clearAbsorptionPlan,
	} = useAbsorption(projectId);

	useMonitorDraggedItem({ projectId });

	useWorkspaceShortcuts({
		inlineRenameBranchFormRef,
		inlineRewordCommitFormRef,
		projectId,
		scope: shortcutScope,
		navigationIndex,
		requestAbsorptionPlan,
		operationMode,
	});

	const dispatch = useAppDispatch();

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	const commit = () =>
		dispatch(
			projectActions.enterMoveMode({
				projectId,
				source: changesSectionItem,
			}),
		);

	return (
		<ProjectPreviewLayout
			projectId={projectId}
			preview={
				selectedItem && (
					<Suspense fallback={<div>Loading preview…</div>}>
						<Preview
							operationMode={operationMode}
							projectId={projectId}
							selectedItem={selectedItem}
						/>
					</Suspense>
				)
			}
		>
			<div className={styles.sections}>
				<div className={styles.changesContainer}>
					<Changes
						operationMode={operationMode}
						projectId={projectId}
						onAbsorbChanges={requestAbsorptionPlan}
						navigationIndex={navigationIndex}
						workspaceMode={workspaceMode}
					/>

					<button type="button" className={uiStyles.button} onClick={commit}>
						Commit
					</button>
				</div>

				{headInfo.stacks.map((stack) => (
					<StackC
						key={stack.id}
						inlineRenameBranchFormRef={inlineRenameBranchFormRef}
						inlineRewordCommitFormRef={inlineRewordCommitFormRef}
						operationMode={operationMode}
						projectId={project.id}
						stack={stack}
						workspaceMode={workspaceMode}
						navigationIndex={navigationIndex}
					/>
				))}

				<div className={styles.section}>
					<BaseCommitRow
						projectId={projectId}
						commitId={getCommonBaseCommitId(headInfo)}
						navigationIndex={navigationIndex}
						operationMode={operationMode}
					/>
				</div>
			</div>

			{shortcutScope && (
				<PositionedShortcutsBar
					label={getScopeLabel(shortcutScope)}
					items={getScopeBindings(shortcutScope)}
				/>
			)}

			{operationMode && (
				<div className={styles.operationModePreview}>
					<OperationSourceLabel headInfo={headInfo} source={operationMode.source} />
				</div>
			)}

			{absorptionPlan && (
				<AbsorptionDialog
					absorptionPlan={absorptionPlan}
					isPending={isAbsorbing}
					onConfirm={confirmAbsorption}
					onOpenChange={(open) => {
						if (!open) clearAbsorptionPlan();
					}}
				/>
			)}
		</ProjectPreviewLayout>
	);
};

export const Route = createRoute({
	getParentRoute: () => projectRoute,
	path: "workspace",
	component: ProjectPage,
});
