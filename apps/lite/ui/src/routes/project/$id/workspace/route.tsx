import {
	commitCreateMutationOptions,
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
import {
	AbsorbIcon,
	DependencyIcon,
	ExpandCollapseIcon,
	MenuTriggerIcon,
	PushIcon,
} from "#ui/components/icons.tsx";
import { rejectedChangesToastOptions } from "#ui/components/RejectedChanges.tsx";
import { type FileParent } from "#ui/domain/FileParent.ts";
import { getBranchNameByCommitId, getCommonBaseCommitId } from "#ui/domain/RefInfo.ts";
import { stackRelativeTo } from "#ui/domain/Stack.ts";
import { ShortcutButton } from "#ui/ShortcutButton.tsx";
import { ProjectPreviewLayout } from "#ui/routes/project/$id/-ProjectPreviewLayout.tsx";
import { getFocus, WorkspaceLayoutContext } from "#ui/state/WorkspaceLayout.tsx";
import {
	BranchSource,
	BranchTarget,
	ChangesSource,
	ChangesTarget,
	CommitFileSource,
	CommitSource,
	CommitTarget,
	ChangesFileSource,
	HunkSource,
	TearOffBranchTarget,
	TreeChangeWithAssignments,
} from "#ui/routes/project/$id/workspace/-OperationSubjects.tsx";
import { AbsorptionDialog, useAbsorption } from "#ui/routes/project/$id/workspace/-Absorption.tsx";
import { useMonitorDraggedOperationSource } from "#ui/routes/project/$id/workspace/-DragAndDrop.tsx";
import {
	CommitDetails as SharedCommitDetails,
	CommitsList,
	FileButton,
	formatHunkHeader,
	HunkDiff,
	Patch,
	CommitLabel,
	shortCommitId,
	assignedHunks,
	assert,
	getRelative,
	hunkKey,
} from "#ui/routes/project/$id/-shared.tsx";
import uiStyles from "#ui/ui.module.css";
import { ContextMenu, Menu, mergeProps, Toast, Tooltip, useRender } from "@base-ui/react";
import {
	AbsorptionTarget,
	Commit,
	DiffHunk,
	DiffSpec,
	HunkAssignment,
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
import { createFileRoute } from "@tanstack/react-router";
import { Array, Match, pipe } from "effect";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";
import {
	ComponentProps,
	FC,
	Fragment,
	ReactNode,
	Ref,
	Suspense,
	use,
	useImperativeHandle,
	useOptimistic,
	useRef,
	useState,
	useTransition,
} from "react";
import useLocalStorageState from "use-local-storage-state";
import sharedStyles from "../-shared.module.css";
import { type Editing } from "./-Editing.ts";
import {
	baseCommitItem,
	changesDetailsItem,
	changesSummaryItem,
	commitItem,
	normalizeItem,
	type Item,
	segmentItem,
	CommitItem,
	ChangesMode,
} from "./-Item.ts";
import { buildNavigationModel } from "./-Selection.ts";
import {
	absorbChangesBinding,
	closeCommitDetailsBinding,
	renameBranchBindings,
	handleRenameBranchKeyDown,
	commitEditingMessageBindings,
	openCommitDetailsBinding,
	handleCommitEditingMessageKeyDown,
	getLabel,
	getScope,
	useWorkspaceShortcuts,
} from "./-WorkspaceShortcuts.ts";
import { PositionedShortcutsBar } from "../-ShortcutsBar.tsx";
import { formatShortcutKeys, ShortcutActionBase, type ShortcutBinding } from "#ui/shortcuts.ts";
import styles from "./route.module.css";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";

type HunkDependencyDiff = HunkDependencies["diffs"][number];
const fileHunkKey = (path: string, hunk: HunkHeader): string => `${path}:${hunkKey(hunk)}`;

const DependencyIndicator: FC<
	{
		projectId: string;
		commitIds: NonEmptyArray<string>;
		onHover: (commitIds: Array<string> | null) => void;
	} & useRender.ComponentProps<"button">
> = ({ projectId, commitIds, onHover, render, ...props }) => {
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
	const trigger = useRender({
		render,
		defaultTagName: "button",
		props: mergeProps<"button">(props, {
			onMouseEnter: () => {
				onHover(commitIds);
			},
			onMouseLeave: () => {
				onHover(null);
			},
			"aria-label": tooltip,
		}),
	});

	return (
		<Tooltip.Root
			// [ref:tooltip-disable-hoverable-popup]
			disableHoverablePopup
		>
			<Tooltip.Trigger render={trigger} />
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

const CommitDetailsC: FC<{
	commitId: string;
	commitSelection: CommitItem;
	projectId: string;
	select: (selection: Item | null) => void;
}> = ({ commitId, commitSelection, projectId, select }) => {
	const selectedPath =
		commitSelection.mode._tag === "Details" ? commitSelection.mode.path : undefined;

	return (
		<SharedCommitDetails
			projectId={projectId}
			commitId={commitId}
			renderFile={(change) => (
				<CommitFileSource
					change={change}
					fileParent={{ _tag: "Commit", commitId }}
					className={classes(
						sharedStyles.item,
						sharedStyles.file,
						selectedPath === change.path && sharedStyles.selectedFile,
					)}
				>
					<FileButton
						change={change}
						onClick={() => {
							select(
								commitItem({
									...commitSelection,
									mode: { _tag: "Details", path: change.path },
								}),
							);
						}}
					/>
				</CommitFileSource>
			)}
		/>
	);
};

// TODO: check this
const assignedChangesDiffSpecs = (
	changes: Array<TreeChange>,
	assignmentsByPath: Map<string, Array<HunkAssignment>>,
): Array<DiffSpec> =>
	changes.flatMap((change) => {
		const assignments = assignmentsByPath.get(change.path);
		if (!assignments || assignments.length === 0) return [];

		if (assignments.some((assignment) => assignment.hunkHeader == null))
			return [createDiffSpec(change, [])];

		return [
			createDiffSpec(
				change,
				assignments.flatMap((assignment) =>
					assignment.hunkHeader != null ? [assignment.hunkHeader] : [],
				),
			),
		];
	});

const hunkContainsHunk = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart <= b.oldStart &&
	a.oldStart + a.oldLines - 1 >= b.oldStart + b.oldLines - 1 &&
	a.newStart <= b.newStart &&
	a.newStart + a.newLines - 1 >= b.newStart + b.newLines - 1;

const getAssignmentsByPath = (
	assignments: Array<HunkAssignment>,
	stackId: string | null,
): Map<string, Array<HunkAssignment>> => {
	const byPath = new Map<string, Array<HunkAssignment>>();

	for (const assignment of assignments) {
		if ((assignment.stackId ?? null) !== stackId) continue;

		const pathAssignments = byPath.get(assignment.path);
		if (pathAssignments) pathAssignments.push(assignment);
		else byPath.set(assignment.path, [assignment]);
	}

	return byPath;
};

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

const dependencyCommitIdsForHunk = (
	hunk: DiffHunk,
	hunkDependencyDiffs: Array<HunkDependencyDiff>,
): Array<string> => {
	const commitIds = new Set<string>();

	for (const [, dependencyHunk, locks] of hunkDependencyDiffs) {
		if (!hunkContainsHunk(hunk, dependencyHunk)) continue;
		for (const dependency of locks) commitIds.add(dependency.commitId);
	}

	return globalThis.Array.from(commitIds);
};

const dependencyCommitIdsForFile = (
	hunkDependencyDiffs: Array<HunkDependencyDiff>,
): Array<string> => {
	const commitIds = new Set<string>();

	for (const [, , locks] of hunkDependencyDiffs)
		for (const dependency of locks) commitIds.add(dependency.commitId);

	return globalThis.Array.from(commitIds);
};

const Hunk: FC<{
	patch: Patch;
	fileParent?: FileParent;
	change: TreeChange;
	hunk: DiffHunk;
	editable: boolean;
	headerStart?: ReactNode;
	onSelectHunk?: (key: string) => void;
	isSelected?: boolean;
	isFocused: boolean;
}> = ({
	patch,
	fileParent,
	change,
	hunk,
	editable,
	headerStart,
	onSelectHunk,
	isSelected,
	isFocused,
}) => {
	const headerRow = (
		<div className={sharedStyles.hunkHeaderRow}>
			{headerStart}
			<div className={sharedStyles.hunkHeader}>{formatHunkHeader(hunk)}</div>
		</div>
	);

	return (
		// oxlint-disable-next-line jsx_a11y/click-events-have-key-events, jsx_a11y/no-static-element-interactions -- TODO
		<div
			onClick={() => onSelectHunk?.(fileHunkKey(change.path, hunk))}
			className={classes(
				sharedStyles.previewHunk,
				isSelected && isFocused && sharedStyles.previewHunkSelected,
			)}
		>
			{fileParent && editable ? (
				<HunkSource patch={patch} fileParent={fileParent} change={change} hunk={hunk}>
					{headerRow}
				</HunkSource>
			) : (
				headerRow
			)}
			<HunkDiff change={change} diff={hunk.diff} />
		</div>
	);
};

const FileDiff: FC<{
	projectId: string;
	change: TreeChange;
	assignments?: Array<HunkAssignment>;
	fileParent?: FileParent;
	editable: boolean;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
	diff: UnifiedPatch | null;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	onSelectHunk: (key: string) => void;
	selectionKey: string | undefined;
	isFocused: boolean;
}> = ({
	projectId,
	change,
	assignments,
	fileParent,
	editable,
	hunkDependencyDiffs,
	diff,
	onDependencyHover,
	onSelectHunk,
	selectionKey,
	isFocused,
}) =>
	Match.value(diff).pipe(
		Match.when(null, () => <div>No diff available for this file.</div>),
		Match.when({ type: "Binary" }, () => <div>Binary file (diff not available).</div>),
		Match.when({ type: "TooLarge" }, ({ subject }) => (
			<div>Diff too large ({subject.sizeInBytes} bytes).</div>
		)),
		Match.when({ type: "Patch" }, (patch) => {
			const visibleHunks = assignments
				? assignedHunks(patch.subject.hunks, assignments)
				: patch.subject.hunks;
			if (visibleHunks.length === 0) return <div>No hunks.</div>;

			return (
				<ul>
					{visibleHunks.map((hunk) => {
						const dependencyCommitIds = hunkDependencyDiffs
							? dependencyCommitIdsForHunk(hunk, hunkDependencyDiffs)
							: [];

						return (
							<li key={hunkKey(hunk)}>
								<Hunk
									patch={patch}
									fileParent={fileParent}
									change={change}
									hunk={hunk}
									editable={editable}
									onSelectHunk={onSelectHunk}
									isSelected={selectionKey === fileHunkKey(change.path, hunk)}
									isFocused={isFocused}
									headerStart={
										fileParent?._tag === "Changes" && isNonEmptyArray(dependencyCommitIds) ? (
											<DependencyIndicator
												projectId={projectId}
												commitIds={dependencyCommitIds}
												onHover={onDependencyHover}
											>
												<DependencyIcon />
											</DependencyIndicator>
										) : undefined
									}
								/>
							</li>
						);
					})}
				</ul>
			);
		}),
		Match.exhaustive,
	);

const hunkKeysFromChangeWithDiff = (
	[change, diff]: [TreeChange, UnifiedPatch | null],
	assignments?: Array<HunkAssignment>,
): Array<string> =>
	diff?.type === "Patch"
		? (assignments ? assignedHunks(diff.subject.hunks, assignments) : diff.subject.hunks).map(
				(hunk) => fileHunkKey(change.path, hunk),
			)
		: [];

type PreviewImperativeHandle = {
	moveSelection: (offset: -1 | 1) => void;
};

const createPreviewImperativeHandle = ({
	hunkKeys,
	selectionKey,
	setSelectionKey,
}: {
	hunkKeys: Array<string>;
	selectionKey: string | undefined;
	setSelectionKey: (key: string | null) => void;
}): PreviewImperativeHandle => ({
	moveSelection: (offset) => {
		if (hunkKeys.length === 0) return;

		const currentKey = selectionKey ?? hunkKeys[0];
		if (currentKey === undefined) return;

		// We assume a valid key was provided.
		const currentIndex = hunkKeys.indexOf(currentKey);

		setSelectionKey(getRelative(hunkKeys, currentIndex, offset));
	},
});

const ChangesPreview: FC<{
	projectId: string;
	stackId: string | null;
	modeSelection: ChangesMode;
	onSelectHunk: (key: string) => void;
	selectionKey: string | null;
	isFocused: boolean;
	setSelectionKey: (key: string | null) => void;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	ref?: Ref<PreviewImperativeHandle>;
}> = ({
	projectId,
	stackId,
	modeSelection,
	onSelectHunk,
	selectionKey,
	isFocused,
	setSelectionKey,
	onDependencyHover,
	ref,
}) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);
	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));
	const selectedPath = modeSelection._tag === "Details" ? modeSelection.path : undefined;
	const selectedChange =
		selectedPath !== undefined
			? changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const visibleChanges = selectedChange ? [selectedChange] : changes;
	const treeChangeDiffs = useSuspenseQueries({
		queries: visibleChanges.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(visibleChanges, Array.zip(treeChangeDiffs));
	const hunkKeys = changesWithDiffs.flatMap(([change, diff]) =>
		hunkKeysFromChangeWithDiff([change, diff], assignmentsByPath.get(change.path)),
	);
	const normalizedSelectionKey =
		selectionKey !== null && hunkKeys.includes(selectionKey) ? selectionKey : hunkKeys[0];
	useImperativeHandle(
		ref,
		() =>
			createPreviewImperativeHandle({
				hunkKeys,
				selectionKey: normalizedSelectionKey,
				setSelectionKey,
			}),
		[normalizedSelectionKey, hunkKeys, setSelectionKey],
	);

	return (
		<div>
			{changesWithDiffs.length === 0 ? (
				<div>No file changes.</div>
			) : (
				<ul>
					{changesWithDiffs.map(([change, diff]) => (
						<li key={change.path}>
							<ChangesFileSource
								change={change}
								fileParent={{ _tag: "Changes", stackId }}
								assignments={assignmentsByPath.get(change.path)}
							>
								<h4>{change.path}</h4>
							</ChangesFileSource>
							<FileDiff
								projectId={projectId}
								change={change}
								fileParent={{ _tag: "Changes", stackId }}
								editable
								assignments={assignmentsByPath.get(change.path)}
								hunkDependencyDiffs={hunkDependencyDiffsByPath.get(change.path)}
								diff={diff}
								onDependencyHover={onDependencyHover}
								onSelectHunk={onSelectHunk}
								selectionKey={normalizedSelectionKey}
								isFocused={isFocused}
							/>
						</li>
					))}
				</ul>
			)}
		</div>
	);
};

const CommitPreview: FC<{
	projectId: string;
	commitId: string;
	selectedPath?: string;
	editable: boolean;
	onSelectHunk: (key: string) => void;
	selectionKey: string | null;
	isFocused: boolean;
	setSelectionKey: (key: string | null) => void;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	ref?: Ref<PreviewImperativeHandle>;
}> = ({
	projectId,
	commitId,
	selectedPath,
	editable,
	onSelectHunk,
	selectionKey,
	isFocused,
	setSelectionKey,
	onDependencyHover,
	ref,
}) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	const selectedChange =
		selectedPath !== undefined
			? commitDetails.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const visibleChanges = selectedChange ? [selectedChange] : commitDetails.changes;
	const treeChangeDiffs = useSuspenseQueries({
		queries: visibleChanges.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(visibleChanges, Array.zip(treeChangeDiffs));
	const hunkKeys = changesWithDiffs.flatMap(([change, diff]) =>
		hunkKeysFromChangeWithDiff([change, diff]),
	);
	const normalizedSelectionKey =
		selectionKey !== null && hunkKeys.includes(selectionKey) ? selectionKey : hunkKeys[0];
	useImperativeHandle(
		ref,
		() =>
			createPreviewImperativeHandle({
				hunkKeys,
				selectionKey: normalizedSelectionKey,
				setSelectionKey,
			}),
		[normalizedSelectionKey, hunkKeys, setSelectionKey],
	);

	return (
		<div>
			{selectedPath === undefined && (
				<>
					<h3>
						<CommitLabel commit={commitDetails.commit} />
					</h3>
					{commitDetails.commit.message.includes("\n") && (
						<p className={sharedStyles.commitMessageBody}>
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
					{changesWithDiffs.map(([change, diff]) => (
						<li key={change.path}>
							{editable ? (
								<CommitFileSource change={change} fileParent={{ _tag: "Commit", commitId }}>
									<h4>{change.path}</h4>
								</CommitFileSource>
							) : (
								<h4>{change.path}</h4>
							)}
							<FileDiff
								projectId={projectId}
								change={change}
								fileParent={{ _tag: "Commit", commitId }}
								editable={editable}
								diff={diff}
								onDependencyHover={onDependencyHover}
								onSelectHunk={onSelectHunk}
								selectionKey={normalizedSelectionKey}
								isFocused={isFocused}
							/>
						</li>
					))}
				</ul>
			)}
		</div>
	);
};

const BranchPreview: FC<{
	projectId: string;
	branchName: string;
	onSelectHunk: (key: string) => void;
	selectionKey: string | null;
	isFocused: boolean;
	setSelectionKey: (key: string | null) => void;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	ref?: Ref<PreviewImperativeHandle>;
}> = ({
	projectId,
	branchName,
	onSelectHunk,
	selectionKey,
	isFocused,
	setSelectionKey,
	onDependencyHover,
	ref,
}) => {
	const [{ data: branchDetails }, { data: branchDiff }] = useSuspenseQueries({
		queries: [
			branchDetailsQueryOptions({ projectId, branchName, remote: null }),
			branchDiffQueryOptions({ projectId, branch: `refs/heads/${branchName}` }),
		],
	});
	const treeChangeDiffs = useSuspenseQueries({
		queries: branchDiff.changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(branchDiff.changes, Array.zip(treeChangeDiffs));
	const hunkKeys = changesWithDiffs.flatMap(([change, diff]) =>
		hunkKeysFromChangeWithDiff([change, diff]),
	);
	const normalizedSelectionKey =
		selectionKey !== null && hunkKeys.includes(selectionKey) ? selectionKey : hunkKeys[0];
	useImperativeHandle(
		ref,
		() =>
			createPreviewImperativeHandle({
				hunkKeys,
				selectionKey: normalizedSelectionKey,
				setSelectionKey,
			}),
		[normalizedSelectionKey, hunkKeys, setSelectionKey],
	);

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
								projectId={projectId}
								change={change}
								editable={false}
								diff={diff}
								onDependencyHover={onDependencyHover}
								onSelectHunk={onSelectHunk}
								selectionKey={normalizedSelectionKey}
								isFocused={isFocused}
							/>
						</li>
					))}
				</ul>
			)}
		</div>
	);
};

const Preview: FC<{
	projectId: string;
	selection: Item;
	onSelectHunk: (key: string) => void;
	selectionKey: string | null;
	isFocused: boolean;
	setSelectionKey: (key: string | null) => void;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	ref?: Ref<PreviewImperativeHandle>;
}> = ({
	projectId,
	selection,
	onSelectHunk,
	selectionKey,
	isFocused,
	setSelectionKey,
	onDependencyHover,
	ref,
}) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Segment: ({ branchName }) => {
				if (branchName == null)
					return (
						<div>
							TODO: the API doesn't provide a way to show details/diffs for segments that don't have
							branch names.
						</div>
					);

				return (
					<BranchPreview
						projectId={projectId}
						branchName={branchName}
						onSelectHunk={onSelectHunk}
						selectionKey={selectionKey}
						isFocused={isFocused}
						setSelectionKey={setSelectionKey}
						onDependencyHover={onDependencyHover}
						ref={ref}
					/>
				);
			},
			Changes: ({ stackId, mode }) => (
				<ChangesPreview
					projectId={projectId}
					stackId={stackId}
					modeSelection={mode}
					onSelectHunk={onSelectHunk}
					selectionKey={selectionKey}
					isFocused={isFocused}
					setSelectionKey={setSelectionKey}
					onDependencyHover={onDependencyHover}
					ref={ref}
				/>
			),
			Commit: (selection) => {
				const selectedPath = selection.mode._tag === "Details" ? selection.mode.path : undefined;
				return (
					<CommitPreview
						projectId={projectId}
						commitId={selection.commitId}
						selectedPath={selectedPath}
						editable
						onSelectHunk={onSelectHunk}
						selectionKey={selectionKey}
						isFocused={isFocused}
						setSelectionKey={setSelectionKey}
						onDependencyHover={onDependencyHover}
						ref={ref}
					/>
				);
			},
			BaseCommit: ({ commitId }) => (
				<CommitPreview
					projectId={projectId}
					commitId={commitId}
					editable={false}
					onSelectHunk={onSelectHunk}
					selectionKey={selectionKey}
					isFocused={isFocused}
					setSelectionKey={setSelectionKey}
					onDependencyHover={onDependencyHover}
					ref={ref}
				/>
			),
		}),
	);

const StackMenuPopup: FC<{
	projectId: string;
	stackId: string;
}> = ({ projectId, stackId }) => {
	const unapplyStack = useMutation(unapplyStackMutationOptions);

	return (
		<Menu.Popup className={classes(uiStyles.popup, uiStyles.menuPopup)}>
			<Menu.Item className={uiStyles.menuItem} disabled>
				Move to leftmost
			</Menu.Item>
			<Menu.Item className={uiStyles.menuItem} disabled>
				Move to rightmost
			</Menu.Item>
			<Menu.Separator />
			<Menu.Item
				className={uiStyles.menuItem}
				disabled={unapplyStack.isPending}
				onClick={() => {
					unapplyStack.mutate({ projectId, stackId });
				}}
			>
				Unapply stack
			</Menu.Item>
		</Menu.Popup>
	);
};

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

const InlineCommitMessageEditor: FC<{
	message: string;
	onSubmit: (value: string) => void;
	onCancel: () => void;
}> = ({ message, onSubmit, onCancel }) => (
	<form
		className={styles.editorForm}
		onSubmit={(event) => {
			event.preventDefault();
			const formData = new FormData(event.currentTarget);
			onCancel();
			onSubmit(formData.get("message") as string);
		}}
	>
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
			className={classes(styles.editorInput, styles.editCommitMessageInput)}
			onKeyDown={(event) => {
				handleCommitEditingMessageKeyDown({
					event: event.nativeEvent,
					onSave: () => event.currentTarget.form?.requestSubmit(),
					onCancel,
				});
			}}
			onBlur={onCancel}
		/>
		<EditorHelp bindings={commitEditingMessageBindings} />
	</form>
);

const CommitMenuPopup: FC<{
	projectId: string;
	commitId: string;
	canReword: boolean;
	onReword: () => void;
	parts: typeof Menu | typeof ContextMenu;
}> = ({ projectId, commitId, canReword, onReword, parts }) => {
	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const commitDiscard = useMutation(commitDiscardMutationOptions);
	const { Popup, Item, SubmenuRoot, SubmenuTrigger, Positioner } = parts;

	return (
		<Popup className={classes(uiStyles.popup, uiStyles.menuPopup)}>
			<Item className={uiStyles.menuItem} disabled={!canReword} onClick={onReword}>
				Reword commit
			</Item>
			<SubmenuRoot>
				<SubmenuTrigger className={uiStyles.menuItem}>Add empty commit</SubmenuTrigger>
				<Positioner>
					<Popup className={classes(uiStyles.popup, uiStyles.menuPopup)}>
						<Item
							className={uiStyles.menuItem}
							onClick={() => {
								commitInsertBlank.mutate({
									projectId,
									relativeTo: { type: "commit", subject: commitId },
									side: "above",
								});
							}}
						>
							Above
						</Item>
						<Item
							className={uiStyles.menuItem}
							onClick={() => {
								commitInsertBlank.mutate({
									projectId,
									relativeTo: { type: "commit", subject: commitId },
									side: "below",
								});
							}}
						>
							Below
						</Item>
					</Popup>
				</Positioner>
			</SubmenuRoot>
			<Item
				className={uiStyles.menuItem}
				disabled={commitDiscard.isPending}
				onClick={() => {
					commitDiscard.mutate({
						projectId,
						subjectCommitId: commitId,
					});
				}}
			>
				Delete commit
			</Item>
		</Popup>
	);
};

const CommitRow: FC<
	{
		branchName: string | null;
		commit: Commit;
		editing: Editing | null;
		isHighlighted: boolean;
		projectId: string;
		segmentIndex: number;
		selection: Item | null;
		select: (selection: Item | null) => void;
		setEditing: (selection: Editing | null) => void;
		stackId: string;
	} & ComponentProps<"div">
> = ({
	branchName,
	commit,
	editing,
	isHighlighted,
	projectId,
	segmentIndex,
	selection,
	select,
	setEditing,
	stackId,
	...restProps
}) => {
	const summaryItem = commitItem({
		stackId,
		segmentIndex,
		branchName,
		commitId: commit.id,
	});
	const commitSelection =
		selection?._tag === "Commit" &&
		selection.stackId === stackId &&
		selection.segmentIndex === segmentIndex &&
		selection.commitId === commit.id
			? selection
			: null;
	const isEditing =
		editing?._tag === "CommitMessage" &&
		editing.subject.stackId === stackId &&
		editing.subject.segmentIndex === segmentIndex &&
		editing.subject.commitId === commit.id;
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);
	const [isCommitMessagePending, startCommitMessageTransition] = useTransition();
	const queryClient = useQueryClient();

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};

	const openDetails = async () => {
		const commitDetails = await queryClient
			.fetchQuery(
				commitDetailsWithLineStatsQueryOptions({
					projectId,
					commitId: commit.id,
				}),
			)
			.catch(() => null);
		if (!commitDetails) return;

		const firstPath = commitDetails.changes[0]?.path;

		select(
			commitItem({
				stackId,
				segmentIndex,
				branchName,
				commitId: commit.id,
				mode: firstPath === undefined ? { _tag: "Details" } : { _tag: "Details", path: firstPath },
			}),
		);
	};

	const toggleDetails = () => {
		setEditing(null);

		if (commitSelection?.mode._tag === "Details") select(summaryItem);
		else void openDetails();
	};

	const commitReword = useMutation(commitRewordMutationOptions);

	const startEditing = () => {
		select(summaryItem);
		setEditing({
			_tag: "CommitMessage",
			subject: {
				stackId,
				segmentIndex,
				branchName,
				commitId: commit.id,
			},
		});
	};

	const endEditing = () => {
		setEditing(null);
	};

	const saveNewMessage = (newMessage: string) => {
		const initialMessage = commit.message.trim();
		const trimmed = newMessage.trim();
		if (trimmed === initialMessage) return;
		startCommitMessageTransition(async () => {
			setOptimisticMessage(trimmed);
			await commitReword
				.mutateAsync({
					projectId,
					commitId: commit.id,
					message: trimmed,
				})
				// Use the global mutation error handler (shows toast) instead of React
				// error boundaries.
				.catch(() => {});
		});
	};

	return (
		<CommitSource
			{...restProps}
			isEnabled={!isEditing}
			commit={commitWithOptimisticMessage}
			className={classes(
				sharedStyles.item,
				commitSelection && sharedStyles.selected,
				isHighlighted && sharedStyles.highlighted,
			)}
		>
			{isEditing ? (
				<InlineCommitMessageEditor
					message={optimisticMessage}
					onSubmit={saveNewMessage}
					onCancel={endEditing}
				/>
			) : (
				<ContextMenu.Root>
					<ContextMenu.Trigger
						render={
							<button
								type="button"
								className={classes(
									sharedStyles.commitButton,
									isCommitMessagePending && sharedStyles.commitButtonPending,
								)}
								onClick={() => {
									select(summaryItem);
								}}
							>
								<CommitLabel commit={commitWithOptimisticMessage} />
							</button>
						}
					/>
					<ContextMenu.Portal>
						<ContextMenu.Positioner>
							<CommitMenuPopup
								projectId={projectId}
								commitId={commit.id}
								canReword={!isCommitMessagePending}
								onReword={startEditing}
								parts={ContextMenu}
							/>
						</ContextMenu.Positioner>
					</ContextMenu.Portal>
				</ContextMenu.Root>
			)}
			<ShortcutButton
				binding={
					commitSelection?.mode._tag === "Details"
						? closeCommitDetailsBinding
						: openCommitDetailsBinding
				}
				className={sharedStyles.itemAction}
				type="button"
				onClick={toggleDetails}
				aria-expanded={commitSelection?.mode._tag === "Details"}
			>
				<ExpandCollapseIcon isExpanded={commitSelection?.mode._tag === "Details"} />
			</ShortcutButton>
			<Menu.Root>
				<Menu.Trigger className={sharedStyles.itemAction} aria-label="Commit menu">
					<MenuTriggerIcon />
				</Menu.Trigger>
				<Menu.Portal>
					<Menu.Positioner align="end">
						<CommitMenuPopup
							projectId={projectId}
							commitId={commit.id}
							canReword={!isCommitMessagePending}
							onReword={startEditing}
							parts={Menu}
						/>
					</Menu.Positioner>
				</Menu.Portal>
			</Menu.Root>
		</CommitSource>
	);
};

const CommitC: FC<{
	branchName: string | null;
	commit: Commit;
	editing: Editing | null;
	isHighlighted: boolean;
	nextCommitId: string | undefined;
	previousCommitId: string | undefined;
	projectId: string;
	segmentIndex: number;
	selection: Item | null;
	select: (selection: Item | null) => void;
	setEditing: (selection: Editing | null) => void;
	stackId: string;
}> = ({
	branchName,
	commit,
	editing,
	isHighlighted,
	nextCommitId,
	previousCommitId,
	projectId,
	segmentIndex,
	selection,
	select,
	setEditing,
	stackId,
}) => {
	const commitSelection =
		selection?._tag === "Commit" &&
		selection.stackId === stackId &&
		selection.segmentIndex === segmentIndex &&
		selection.commitId === commit.id
			? selection
			: null;

	return (
		<CommitTarget
			commitId={commit.id}
			previousCommitId={previousCommitId}
			nextCommitId={nextCommitId}
		>
			<CommitRow
				branchName={branchName}
				commit={commit}
				editing={editing}
				isHighlighted={isHighlighted}
				projectId={projectId}
				segmentIndex={segmentIndex}
				selection={selection}
				select={select}
				setEditing={setEditing}
				stackId={stackId}
			/>
			{commitSelection?.mode._tag === "Details" && (
				<Suspense fallback={<div className={sharedStyles.itemEmpty}>Loading change details…</div>}>
					<CommitDetailsC
						commitSelection={commitSelection}
						projectId={projectId}
						commitId={commit.id}
						select={select}
					/>
				</Suspense>
			)}
		</CommitTarget>
	);
};

const Changes: FC<{
	label: string;
	projectId: string;
	stackId: string | null;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	selection: Item | null;
	select: (selection: Item | null) => void;
	className?: string;
}> = ({
	label,
	projectId,
	stackId,
	onAbsorbChanges,
	onDependencyHover,
	selection,
	select,
	className,
}) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));
	const changesSelection =
		selection?._tag === "Changes" && selection.stackId === stackId ? selection : null;

	return (
		<ChangesSource
			stackId={stackId}
			label={label}
			changes={changes.map(
				(change): TreeChangeWithAssignments => ({
					change,
					assignments: assignmentsByPath.get(change.path),
				}),
			)}
			render={
				<ChangesTarget
					stackId={stackId}
					className={classes(className, changesSelection && sharedStyles.sectionSelected)}
				/>
			}
		>
			<div
				className={classes(
					sharedStyles.item,
					changesSelection?.mode._tag === "Summary" && sharedStyles.selected,
				)}
			>
				<button
					type="button"
					className={styles.segmentButton}
					onClick={() => {
						select(changesSummaryItem(stackId));
					}}
				>
					{label}
				</button>
				<ShortcutButton
					binding={absorbChangesBinding}
					type="button"
					className={sharedStyles.itemAction}
					disabled={changes.length === 0}
					onClick={() => {
						onAbsorbChanges({
							type: "treeChanges",
							subject: {
								changes,
								assigned_stack_id: stackId,
							},
						});
					}}
				>
					<AbsorbIcon />
				</ShortcutButton>
				<Menu.Root>
					<Menu.Trigger className={sharedStyles.itemAction} aria-label={`${label} menu`}>
						<MenuTriggerIcon />
					</Menu.Trigger>
					<Menu.Portal>
						<Menu.Positioner align="end">
							<Menu.Popup className={classes(uiStyles.popup, uiStyles.menuPopup)}>
								<Menu.Item
									className={uiStyles.menuItem}
									disabled={changes.length === 0}
									onClick={() => {
										onAbsorbChanges({
											type: "treeChanges",
											subject: {
												changes,
												assigned_stack_id: stackId,
											},
										});
									}}
								>
									Absorb all changes
								</Menu.Item>
							</Menu.Popup>
						</Menu.Positioner>
					</Menu.Portal>
				</Menu.Root>
			</div>
			{changes.length === 0 ? (
				<div className={sharedStyles.itemEmpty}>No changes.</div>
			) : (
				<ul>
					{changes.map((change) => {
						const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
						const dependencyCommitIds = hunkDependencyDiffs
							? dependencyCommitIdsForFile(hunkDependencyDiffs)
							: [];

						return (
							<li key={change.path}>
								<ChangesFileSource
									change={change}
									fileParent={{ _tag: "Changes", stackId }}
									assignments={assignmentsByPath.get(change.path)}
									className={classes(
										sharedStyles.item,
										changesSelection?.mode._tag === "Details" &&
											changesSelection.mode.path === change.path &&
											sharedStyles.selected,
									)}
								>
									<FileButton
										change={change}
										onClick={() => {
											select(changesDetailsItem(stackId, change.path));
										}}
									/>
									<ShortcutButton
										binding={absorbChangesBinding}
										type="button"
										className={sharedStyles.itemAction}
										onClick={() => {
											onAbsorbChanges({
												type: "treeChanges",
												subject: {
													changes: [change],
													assigned_stack_id: stackId,
												},
											});
										}}
									>
										<AbsorbIcon />
									</ShortcutButton>
									{isNonEmptyArray(dependencyCommitIds) && (
										<DependencyIndicator
											projectId={projectId}
											commitIds={dependencyCommitIds}
											onHover={onDependencyHover}
											className={sharedStyles.itemAction}
										>
											<DependencyIcon />
										</DependencyIndicator>
									)}
								</ChangesFileSource>
							</li>
						);
					})}
				</ul>
			)}
		</ChangesSource>
	);
};

const CommitForm: FC<{
	projectId: string;
	stack: Stack;
}> = ({ projectId, stack }) => {
	const [message, setMessage] = useLocalStorageState(
		// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
		`project:${projectId}:commitMessage:${stack.id!}`,
		{ defaultValue: "" },
	);
	const toastManager = Toast.useToastManager();
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const relativeTo = stackRelativeTo(stack);
	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stack.id);
	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));
	const diffSpecs = assignedChangesDiffSpecs(changes, assignmentsByPath);

	const commitCreate = useMutation(commitCreateMutationOptions);

	const disabled = commitCreate.isPending || !relativeTo;

	return (
		<form
			className={styles.commitForm}
			onSubmit={(event) => {
				event.preventDefault();
				if (disabled) return;
				commitCreate.mutate(
					{
						projectId,
						relativeTo,
						side: "below",
						changes: diffSpecs,
						message: message.trim(),
					},
					{
						onSuccess: (response) => {
							if (response.rejectedChanges.length > 0)
								toastManager.add(
									rejectedChangesToastOptions({
										newCommit: response.newCommit,
										rejectedChanges: response.rejectedChanges,
									}),
								);

							setMessage("");
						},
					},
				);
			}}
		>
			<textarea
				// TODO: inline editor uses enter to submit, this doesn't
				aria-label="Commit message"
				className={styles.commitFormMessageInput}
				placeholder="Commit message"
				value={message}
				onChange={(event) => {
					setMessage(event.target.value);
				}}
				onKeyDown={(event) => {
					if (event.key === "Enter" && event.metaKey) {
						event.preventDefault();
						if (!disabled) event.currentTarget.form?.requestSubmit();
					}
				}}
			/>
			<button type="submit" disabled={disabled} className={uiStyles.button}>
				Commit
			</button>
		</form>
	);
};

const SegmentMenuPopup: FC<{
	canRename: boolean;
	onRename: () => void;
	parts: typeof Menu | typeof ContextMenu;
}> = ({ canRename, onRename, parts }) => {
	const { Popup, Item } = parts;

	return (
		<Popup className={classes(uiStyles.popup, uiStyles.menuPopup)}>
			<Item className={uiStyles.menuItem} disabled={!canRename} onClick={onRename}>
				Rename branch
			</Item>
		</Popup>
	);
};

const InlineBranchNameEditor: FC<{
	branchName: string;
	onSubmit: (value: string) => void;
	onExit: () => void;
}> = ({ branchName, onSubmit, onExit }) => (
	<form
		className={styles.editorForm}
		onSubmit={(event) => {
			event.preventDefault();
			const formData = new FormData(event.currentTarget);
			onExit();
			onSubmit(formData.get("branchName") as string);
		}}
	>
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
			onKeyDown={(event) => {
				handleRenameBranchKeyDown({
					event: event.nativeEvent,
					onSave: () => event.currentTarget.form?.requestSubmit(),
					onCancel: onExit,
				});
			}}
			onBlur={onExit}
		/>
		<EditorHelp bindings={renameBranchBindings} />
	</form>
);

const SegmentRow: FC<
	{
		projectId: string;
		editing: Editing | null;
		segment: Segment;
		stackId: string;
		segmentIndex: number;
		selection: Item | null;
		select: (selection: Item | null) => void;
		setEditing: (selection: Editing | null) => void;
	} & ComponentProps<"div">
> = ({
	projectId,
	editing,
	segment,
	stackId,
	segmentIndex,
	selection,
	select,
	setEditing,
	...restProps
}) => {
	const branchName = segment.refName?.displayName ?? null;
	const segmentItemV = segmentItem({
		stackId,
		segmentIndex,
		branchName,
	});
	const segmentSelection =
		selection?._tag === "Segment" &&
		selection.stackId === stackId &&
		selection.segmentIndex === segmentIndex
			? selection
			: null;
	const isEditing =
		branchName !== null &&
		editing?._tag === "BranchName" &&
		editing.subject.stackId === stackId &&
		editing.subject.segmentIndex === segmentIndex;
	const [optimisticBranchName, setOptimisticBranchName] = useOptimistic(
		branchName,
		(_currentBranchName, nextBranchName: string) => nextBranchName,
	);
	const [isRenamePending, startRenameTransition] = useTransition();

	const updateBranchName = useMutation(updateBranchNameMutationOptions);

	const startEditing = () => {
		if (branchName === null) return;
		select(segmentItemV);
		setEditing({
			_tag: "BranchName",
			subject: { stackId, segmentIndex },
		});
	};

	const endEditing = () => {
		setEditing(null);
	};

	const saveBranchName = (newBranchName: string) => {
		if (branchName === null) return;
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
			select(
				segmentItem({
					stackId,
					segmentIndex,
					branchName: trimmed,
				}),
			);
		});
	};

	const children = (
		<div
			{...restProps}
			className={classes(
				restProps.className,
				sharedStyles.item,
				segmentSelection && sharedStyles.selected,
			)}
		>
			{isEditing && optimisticBranchName !== null ? (
				<InlineBranchNameEditor
					branchName={optimisticBranchName}
					onSubmit={saveBranchName}
					onExit={endEditing}
				/>
			) : (
				<ContextMenu.Root>
					<ContextMenu.Trigger
						render={
							<button
								type="button"
								className={styles.segmentButton}
								onClick={() => select(segmentItemV)}
							>
								{optimisticBranchName ?? "Untitled"}
							</button>
						}
					/>
					<ContextMenu.Portal>
						<ContextMenu.Positioner>
							<SegmentMenuPopup
								canRename={branchName !== null && !isRenamePending}
								onRename={startEditing}
								parts={ContextMenu}
							/>
						</ContextMenu.Positioner>
					</ContextMenu.Portal>
				</ContextMenu.Root>
			)}
			<button type="button" className={sharedStyles.itemAction} aria-label="Push branch" disabled>
				<PushIcon />
			</button>
			<Menu.Root>
				<Menu.Trigger className={sharedStyles.itemAction} aria-label="Branch menu">
					<MenuTriggerIcon />
				</Menu.Trigger>
				<Menu.Portal>
					<Menu.Positioner align="end">
						<SegmentMenuPopup
							canRename={branchName !== null && !isRenamePending}
							onRename={startEditing}
							parts={Menu}
						/>
					</Menu.Positioner>
				</Menu.Portal>
			</Menu.Root>
		</div>
	);

	return !isRenamePending && segment.refName != null ? (
		<BranchTarget
			branchRef={segment.refName.fullNameBytes}
			firstCommitId={segment.commits[0]?.id}
			render={
				<BranchSource
					branchRef={segment.refName.fullNameBytes}
					branchName={segment.refName.displayName}
					render={children}
				/>
			}
		/>
	) : (
		children
	);
};

const SegmentC: FC<{
	highlightedCommitIds: Set<string>;
	projectId: string;
	segment: Segment;
	segmentIndex: number;
	selection: Item | null;
	select: (selection: Item | null) => void;
	editing: Editing | null;
	setEditing: (selection: Editing | null) => void;
	stackId: string;
}> = ({
	editing,
	highlightedCommitIds,
	projectId,
	segment,
	segmentIndex,
	selection,
	select,
	setEditing,
	stackId,
}) => {
	const isSelected =
		(selection?._tag === "Segment" &&
			selection.stackId === stackId &&
			selection.segmentIndex === segmentIndex) ||
		(selection?._tag === "Commit" &&
			selection.stackId === stackId &&
			segment.commits.some((commit) => commit.id === selection.commitId));

	return (
		<div className={classes(isSelected && sharedStyles.sectionSelected)}>
			<SegmentRow
				projectId={projectId}
				editing={editing}
				segment={segment}
				stackId={stackId}
				segmentIndex={segmentIndex}
				selection={selection}
				select={select}
				setEditing={setEditing}
			/>

			<CommitsList commits={segment.commits}>
				{(commit, index) => (
					<CommitC
						branchName={segment.refName?.displayName ?? null}
						commit={commit}
						editing={editing}
						isHighlighted={highlightedCommitIds.has(commit.id)}
						nextCommitId={segment.commits[index + 1]?.id}
						previousCommitId={segment.commits[index - 1]?.id}
						projectId={projectId}
						segmentIndex={segmentIndex}
						selection={selection}
						select={select}
						setEditing={setEditing}
						stackId={stackId}
					/>
				)}
			</CommitsList>
		</div>
	);
};

const StackC: FC<{
	highlightedCommitIds: Set<string>;
	onAbsorbChanges: (target: AbsorptionTarget) => void;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	projectId: string;
	selection: Item | null;
	select: (selection: Item | null) => void;
	editing: Editing | null;
	setEditing: (selection: Editing | null) => void;
	stack: Stack;
}> = ({
	editing,
	highlightedCommitIds,
	onAbsorbChanges,
	onDependencyHover,
	projectId,
	selection,
	select,
	setEditing,
	stack,
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
		<div className={styles.stack}>
			<div>
				<div className={styles.stackHeader}>
					<Menu.Root>
						<Menu.Trigger className={styles.stackMenuTrigger} aria-label="Stack menu">
							<MenuTriggerIcon />
						</Menu.Trigger>
						<Menu.Portal>
							<Menu.Positioner align="end">
								<StackMenuPopup projectId={projectId} stackId={stackId} />
							</Menu.Positioner>
						</Menu.Portal>
					</Menu.Root>
				</div>
				<Changes
					label="Assigned changes"
					projectId={projectId}
					stackId={stack.id}
					onAbsorbChanges={onAbsorbChanges}
					onDependencyHover={onDependencyHover}
					selection={selection}
					select={select}
					className={styles.assignedChanges}
				/>
				<CommitForm projectId={projectId} stack={stack} />
			</div>

			<ul className={styles.segments}>
				{stack.segments.map((segment, segmentIndex) => (
					// oxlint-disable-next-line react/no-array-index-key -- It's all we have.
					<li key={segmentIndex}>
						<SegmentC
							editing={editing}
							highlightedCommitIds={highlightedCommitIds}
							projectId={projectId}
							segment={segment}
							segmentIndex={segmentIndex}
							selection={selection}
							select={select}
							setEditing={setEditing}
							stackId={stackId}
						/>
					</li>
				))}
			</ul>
		</div>
	);
};

const ProjectPage: FC = () => {
	const { id: projectId } = Route.useParams();
	const [layoutState, dispatchLayout] = assert(use(WorkspaceLayoutContext));

	const [highlightedCommitIds, setHighlightedCommitIds] = useState<Set<string>>(() => new Set());
	const [editing, setEditingState] = useState<Editing | null>(null);
	const [previewSelectionKey, setPreviewSelectionKey] = useState<string | null>(null);
	const previewRef = useRef<PreviewImperativeHandle | null>(null);

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());

	const project = projects.find((project) => project.id === projectId);

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const [_selection, setSelection] = useState<Item | null>(null);
	const commonBaseCommitId = getCommonBaseCommitId(headInfo);
	const navigationModel = buildNavigationModel({
		headInfo,
		changes: worktreeChanges.changes,
		assignments: worktreeChanges.assignments,
		commonBaseCommitId,
	});
	const selection =
		(_selection ? normalizeItem(_selection, headInfo, worktreeChanges) : null) ??
		navigationModel.items[0] ??
		null;

	const select = (nextSelection: Item | null) => {
		dispatchLayout({ _tag: "FocusPrimary" });
		setPreviewSelectionKey(null);
		setSelection(nextSelection);
	};
	const setEditing = (nextEditing: Editing | null) => {
		dispatchLayout({ _tag: "FocusPrimary" });
		setEditingState(nextEditing);
	};
	const highlightCommits = (commitIds: Array<string> | null) => {
		setHighlightedCommitIds(commitIds ? new Set(commitIds) : new Set());
	};

	const movePreviewSelection = (offset: -1 | 1) => {
		previewRef.current?.moveSelection(offset);
	};
	const onSelectPreviewHunk = (key: string) => {
		dispatchLayout({ _tag: "FocusPreview" });
		setPreviewSelectionKey(key);
	};

	const shortcutScope = getScope({
		selection,
		editing,
		layoutState,
	});

	const {
		absorptionPlan,
		isAbsorbing,
		requestAbsorptionPlan,
		confirmAbsorption,
		clearAbsorptionPlan,
	} = useAbsorption(projectId);

	useMonitorDraggedOperationSource({ projectId });
	useWorkspaceShortcuts({
		projectId,
		scope: shortcutScope,
		select,
		setEditing,
		navigationModel,
		requestAbsorptionPlan,
		dispatchLayout,
		movePreviewSelection,
	});

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	return (
		<ProjectPreviewLayout
			projectId={projectId}
			preview={
				selection && (
					<Suspense fallback={<div>Loading preview…</div>}>
						<Preview
							projectId={projectId}
							selection={selection}
							onSelectHunk={onSelectPreviewHunk}
							selectionKey={previewSelectionKey}
							isFocused={getFocus(layoutState) === "preview"}
							setSelectionKey={setPreviewSelectionKey}
							onDependencyHover={highlightCommits}
							ref={previewRef}
						/>
					</Suspense>
				)
			}
		>
			<div className={sharedStyles.lanes}>
				<Changes
					label="Unassigned changes"
					projectId={project.id}
					stackId={null}
					onAbsorbChanges={requestAbsorptionPlan}
					onDependencyHover={highlightCommits}
					selection={selection}
					select={select}
					className={styles.unassignedChanges}
				/>

				<div className={styles.headInfo}>
					<div className={styles.stackLanes}>
						{headInfo.stacks.map((stack) => (
							<div key={stack.id} className={styles.stackLane}>
								<StackC
									editing={editing}
									highlightedCommitIds={highlightedCommitIds}
									onAbsorbChanges={requestAbsorptionPlan}
									onDependencyHover={highlightCommits}
									projectId={project.id}
									selection={selection}
									select={select}
									setEditing={setEditing}
									stack={stack}
								/>
							</div>
						))}
					</div>

					{commonBaseCommitId !== undefined && (
						<TearOffBranchTarget className={styles.commonBaseCommitContainer}>
							<div
								className={classes(
									sharedStyles.item,
									selection?._tag === "BaseCommit" &&
										selection.commitId === commonBaseCommitId &&
										sharedStyles.selected,
								)}
							>
								<button
									type="button"
									className={styles.commonBaseCommit}
									onClick={() => {
										select(baseCommitItem(commonBaseCommitId));
										setEditing(null);
									}}
								>
									{shortCommitId(commonBaseCommitId)} (common base commit)
								</button>
							</div>
						</TearOffBranchTarget>
					)}
				</div>

				<TearOffBranchTarget className={styles.emptyLane} />
			</div>

			<PositionedShortcutsBar
				label={shortcutScope ? getLabel(shortcutScope) : null}
				items={shortcutScope?.bindings ?? []}
			/>
			{absorptionPlan !== null && (
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

export const Route = createFileRoute("/project/$id/workspace")({
	component: ProjectPage,
});
