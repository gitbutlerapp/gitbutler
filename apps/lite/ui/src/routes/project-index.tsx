import useLocalStorageState from "use-local-storage-state";
import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	Menu,
	mergeProps,
	Popover,
	Toast,
	type ToastManagerAddOptions,
	Tooltip,
	useRender,
} from "@base-ui/react";
import { createRoute } from "@tanstack/react-router";
import {
	RefInfo,
	Commit,
	DiffHunk,
	HunkAssignment,
	InsertSide,
	TreeChange,
	DiffSpec,
	RelativeTo,
	Stack,
	HunkDependencies,
	HunkHeader,
} from "@gitbutler/but-sdk";
import { Array, Match } from "effect";
import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import { createContext, FC, Suspense, useContext, useEffect, useState } from "react";
import styles from "./project-index.module.css";
import sharedStyles from "./project-shared.module.css";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import { useDroppable } from "#ui/hooks/useDroppable.ts";
import { ProjectPanelLayout } from "#ui/routes/ProjectPanelLayout.tsx";
import {
	CommitDetails,
	CommitLabel,
	CommitRow,
	CommitsList,
	type DragData,
	DragPreview,
	FileButton,
	FileDiff,
	Hunk,
	type SourceItem,
} from "#ui/routes/project-shared.tsx";
import {
	commitMoveMutationOptions,
	commitCreateMutationOptions,
	rubMutationOptions,
	unapplyStackMutationOptions,
} from "#ui/mutations.ts";
import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/queries.ts";
import { type ChangeUnit } from "#ui/ChangeUnit.ts";
import { RejectedChange, RejectedChanges } from "#ui/components/RejectedChanges.tsx";
import { rubOperationLabel, RubParams, type RubSource } from "#ui/rub.ts";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { createDiffSpec } from "#ui/DiffSpec.ts";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";
import { CommitMoveParams } from "#electron/ipc.ts";

const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

type HunkDependencyDiff = HunkDependencies["diffs"][number];

const getBranchNameByCommitId = (headInfo: RefInfo): Map<string, string> => {
	const byCommitId = new Map<string, string>();

	for (const stack of headInfo.stacks)
		for (const segment of stack.segments) {
			const branchName = segment.refName?.displayName ?? "Untitled";
			for (const commit of segment.commits) byCommitId.set(commit.id, branchName);
		}

	return byCommitId;
};

const getStackIdsByCommitId = (headInfo: RefInfo): Map<string, Set<string>> => {
	const byCommitId = new Map<string, Set<string>>();

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;

		for (const segment of stack.segments)
			for (const commit of segment.commits) {
				const stackIds = byCommitId.get(commit.id) ?? new Set<string>();
				stackIds.add(stack.id);
				byCommitId.set(commit.id, stackIds);
			}
	}

	return byCommitId;
};

const DependencyIndicator: FC<{
	projectId: string;
	commitIds: NonEmptyArray<string>;
	onHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, commitIds, onHover }) => {
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	// TODO: expensive
	const branchNameByCommitId = getBranchNameByCommitId(headInfo);
	const branchNames = Array.flatMapNullable(commitIds, (commitId) =>
		branchNameByCommitId.get(commitId),
	);
	const tooltip =
		branchNames.length > 0 ? `Depends on ${branchNames.join(", ")}` : "Unknown dependencies";

	return (
		<Popover.Root>
			<Popover.Trigger
				className={styles.dependencyIndicator}
				openOnHover
				onMouseEnter={() => {
					onHover(commitIds);
				}}
				onMouseLeave={() => {
					onHover(null);
				}}
				aria-label={tooltip}
				style={{ lineHeight: 1 }}
			>
				🔗
			</Popover.Trigger>
			<Popover.Portal>
				<Popover.Positioner sideOffset={8}>
					<Popover.Popup className={styles.tooltip}>{tooltip}</Popover.Popup>
				</Popover.Positioner>
			</Popover.Portal>
		</Popover.Root>
	);
};

/**
 * @example
 * classes('foo', undefined, 'bar', '', 'baz') === 'foo bar baz'
 */
const classes = (...xs: Array<string | null | undefined | false>): string =>
	// oxlint-disable-next-line typescript/strict-boolean-expressions
	xs.reduce((acc: string, x) => (x ? (acc ? `${acc} ${x}` : x) : acc), "");

const rejectedChangesToastOptions = ({
	newCommit,
	pathsToRejectedChanges,
}: {
	newCommit?: string | null;
	pathsToRejectedChanges: Array<RejectedChange>;
}): ToastManagerAddOptions<never> => ({
	title: newCommit != null ? "Some changes were not committed" : "Failed to create commit",
	description: <RejectedChanges rejectedChanges={pathsToRejectedChanges} />,
	priority: "high",
});

const commonBaseCommitId = (headInfo: RefInfo): string | undefined => {
	const bases = headInfo.stacks
		.map((stack) => stack.base)
		.filter((base): base is string => base !== null);
	const first = bases[0];
	if (first === undefined) return undefined;
	return bases.every((base) => base === first) ? first : undefined;
};

const rubSourceFor = (item: SourceItem): RubSource | null =>
	Match.value(item).pipe(
		Match.tag("Commit", ({ commitId }): RubSource | null => ({
			_tag: "Commit",
			source: { commitId },
		})),
		Match.tag("TreeChange", ({ source }): RubSource | null => ({
			_tag: "TreeChange",
			source,
		})),
		Match.exhaustive,
	);

const DraggedSourceItemContext = createContext<SourceItem | null>(null);

const parseDragData = (data: unknown): SourceItem | null => {
	if (typeof data !== "object" || data === null || !("sourceItem" in data)) return null;
	return (data as DragData).sourceItem;
};

const useDraggedSourceItem = (): SourceItem | null => useContext(DraggedSourceItemContext);

type OperationTarget =
	| ({
			_tag: "Rub";
	  } & Omit<RubParams, "projectId" | "source">)
	| ({
			_tag: "CommitMove";
	  } & Omit<CommitMoveParams, "projectId" | "subjectCommitId">);

const parseDropTargetData = (data: unknown): OperationTarget | null => {
	if (typeof data !== "object" || data === null || !("_tag" in data)) return null;
	return data as OperationTarget;
};

const useMonitorDraggedSourceItem = ({
	projectId,
	setDraggedSourceItem,
}: {
	projectId: string;
	setDraggedSourceItem: (sourceItem: SourceItem | null) => void;
}): void => {
	const runOperation = useRunOperation(projectId);

	useEffect(
		() =>
			monitorForElements({
				canMonitor: ({ source }) => parseDragData(source.data) !== null,
				onDragStart: ({ source }) => {
					setDraggedSourceItem(parseDragData(source.data));
				},
				onDrop: ({ source, location }) => {
					setDraggedSourceItem(null);

					const sourceItem = parseDragData(source.data);
					const operationTarget = location.current.dropTargets
						.map((dropTarget) => parseDropTargetData(dropTarget.data))
						.find((target) => target);

					if (!sourceItem || !operationTarget) return;

					runOperation(sourceItem, operationTarget);
				},
			}),
		[runOperation, setDraggedSourceItem],
	);
};

const useRunOperation = (projectId: string) => {
	const toastManager = Toast.useToastManager();
	const rubMutation = useMutation(rubMutationOptions);
	const commitMove = useMutation(commitMoveMutationOptions);

	return (sourceItem: SourceItem, operationTarget: OperationTarget): void => {
		Match.value(operationTarget).pipe(
			Match.tag("Rub", (operationTarget) => {
				const rubSource = rubSourceFor(sourceItem);
				if (!rubSource) return;
				rubMutation.mutate(
					{
						projectId,
						source: rubSource,
						target: operationTarget.target,
					},
					{
						onSuccess: (response) => {
							const pathsToRejectedChanges = response.pathsToRejectedChanges ?? [];
							if (pathsToRejectedChanges.length > 0)
								toastManager.add(
									rejectedChangesToastOptions({
										newCommit: response.newCommit,
										// Assertion is temporary until API response types have been fixed.
										pathsToRejectedChanges:
											response.pathsToRejectedChanges as Array<RejectedChange>,
									}),
								);
							return;
						},
					},
				);
			}),
			Match.tag("CommitMove", (operationTarget) => {
				if (sourceItem._tag !== "Commit") return;
				commitMove.mutate({
					projectId,
					subjectCommitId: sourceItem.commitId,
					relativeTo: operationTarget.relativeTo,
					side: operationTarget.side,
				});
			}),
			Match.exhaustive,
		);
	};
};

const stackRelativeTo = (stack: Stack): RelativeTo | null => {
	const segmentWithRef = stack.segments.find((segment) => segment.refName != null);
	if (segmentWithRef?.refName)
		return {
			type: "referenceBytes",
			subject: segmentWithRef.refName.fullNameBytes,
		};

	const firstCommit = stack.segments.flatMap((segment) => segment.commits)[0];
	if (!firstCommit) return null;

	return { type: "commit", subject: firstCommit.id };
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

const DraggableFile: FC<
	{
		change: TreeChange;
		changeUnit: ChangeUnit;
		assignments?: Array<HunkAssignment>;
	} & useRender.ComponentProps<"div">
> = ({ change, changeUnit, assignments, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({
			sourceItem: {
				_tag: "TreeChange",
				source: {
					parent: changeUnit,
					change,
					hunkHeaders: assignments
						? assignments.flatMap((assignment) =>
								// TODO: is this correct?
								assignment.hunkHeader != null ? [assignment.hunkHeader] : [],
							)
						: [],
				},
			},
		}),
		preview: <DragPreview>{change.path}</DragPreview>,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && sharedStyles.dragging),
		}),
	});
};

const ChangesFileDiff: FC<{
	projectId: string;
	stackId: string | null;
	path: string;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, stackId, path, onDependencyHover }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const assignments = assignmentsByPath.get(path);
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);
	const change = worktreeChanges.changes.find((candidate) => candidate.path === path);

	if (!assignments || !change) return null;

	return (
		<FileDiff
			projectId={projectId}
			change={change}
			assignments={assignments}
			renderHunk={(hunk, patch) => {
				const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(path);

				const dependencyCommitIds = hunkDependencyDiffs
					? dependencyCommitIdsForHunk(hunk, hunkDependencyDiffs)
					: [];

				return (
					<Hunk
						patch={patch}
						changeUnit={{ _tag: "changes", stackId }}
						change={change}
						hunk={hunk}
						headerStart={
							isNonEmptyArray(dependencyCommitIds) && (
								<DependencyIndicator
									projectId={projectId}
									commitIds={dependencyCommitIds}
									onHover={onDependencyHover}
								/>
							)
						}
					/>
				);
			}}
		/>
	);
};

const CommitFileDiff: FC<{
	projectId: string;
	commitId: string;
	path: string;
}> = ({ projectId, commitId, path }) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	const change = data.changes.find((candidate) => candidate.path === path);

	if (!change) return null;

	return (
		<FileDiff
			projectId={projectId}
			change={change}
			renderHunk={(hunk, patch) => (
				<Hunk patch={patch} changeUnit={{ _tag: "commit", commitId }} change={change} hunk={hunk} />
			)}
		/>
	);
};

const ShowCommit: FC<{
	projectId: string;
	commitId: string;
}> = ({ projectId, commitId }) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	if (data.changes.length === 0) return null;

	const firstLineEnd = data.commit.message.indexOf("\n");
	const commitMessageBody =
		firstLineEnd === -1 ? "" : data.commit.message.slice(firstLineEnd + 1).trim();

	return (
		<>
			<h3>
				<CommitLabel commit={data.commit} />
			</h3>
			{commitMessageBody !== "" && (
				<p className={styles.selectedCommitMessageBody}>{commitMessageBody}</p>
			)}
			<ul className={sharedStyles.hunks}>
				{data.changes.map((change) => (
					<li key={change.path}>
						<h4>{change.path}</h4>
						<FileDiff
							projectId={projectId}
							change={change}
							renderHunk={(hunk, patch) => (
								<Hunk
									patch={patch}
									changeUnit={{ _tag: "commit", commitId }}
									change={change}
									hunk={hunk}
								/>
							)}
						/>
					</li>
				))}
			</ul>
		</>
	);
};

type Selection =
	| {
			_tag: "changes";
			stackId: string | null;
			path: string;
	  }
	| {
			_tag: "commit";
			stackId: string;
			commitId: string;
			path?: string;
	  };

const normalizeSelection = (
	selection: Selection,
	stackIdsByCommitId: Map<string, Set<string>>,
): Selection | null =>
	Match.value(selection).pipe(
		Match.tag("changes", (selection) => selection),
		Match.tag("commit", (selection) => {
			const stackIds = stackIdsByCommitId.get(selection.commitId);
			if (stackIds === undefined) return null;
			if (!stackIds.has(selection.stackId)) return null;
			return selection;
		}),
		Match.exhaustive,
	);

const firstSelectablePath = ({
	changes,
	assignments,
	stackId,
}: {
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
	stackId: string | null;
}): string | null => {
	const assignmentsByPath = getAssignmentsByPath(assignments, stackId);
	return changes.find((change) => assignmentsByPath.has(change.path))?.path ?? null;
};

const getDefaultSelection = ({
	headInfo,
	changes,
	assignments,
}: {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
}): Selection | null => {
	const firstUnassignedPath = firstSelectablePath({ changes, assignments, stackId: null });
	if (firstUnassignedPath !== null)
		return { _tag: "changes", stackId: null, path: firstUnassignedPath };

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;

		const firstAssignedPath = firstSelectablePath({
			changes,
			assignments,
			stackId: stack.id,
		});
		if (firstAssignedPath !== null)
			return { _tag: "changes", stackId: stack.id, path: firstAssignedPath };

		for (const segment of stack.segments) {
			const firstCommit = segment.commits[0];
			if (firstCommit) return { _tag: "commit", stackId: stack.id, commitId: firstCommit.id };
		}
	}

	return null;
};

const Preview: FC<{
	projectId: string;
	selection: Selection;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, selection, onDependencyHover }) =>
	Match.value(selection).pipe(
		Match.tag("changes", ({ stackId, path }) => (
			<ChangesFileDiff
				projectId={projectId}
				stackId={stackId}
				path={path}
				onDependencyHover={onDependencyHover}
			/>
		)),
		Match.tag("commit", ({ commitId, path }) =>
			path !== undefined ? (
				<CommitFileDiff projectId={projectId} commitId={commitId} path={path} />
			) : (
				<ShowCommit projectId={projectId} commitId={commitId} />
			),
		),
		Match.exhaustive,
	);

const RubTarget: FC<
	{
		target: ChangeUnit;
	} & useRender.ComponentProps<"div">
> = ({ target, render, ...props }) => {
	const sourceItem = useDraggedSourceItem();
	const [isDropTarget, dropRef] = useDroppable({
		canDrop: ({ source }) => {
			const sourceItem = parseDragData(source.data);
			const rubSource = sourceItem ? rubSourceFor(sourceItem) : null;
			return rubSource !== null && rubOperationLabel(rubSource, target) !== null;
		},
		getData: (): OperationTarget => ({
			_tag: "Rub",
			target,
		}),
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps(props, {
			style: { ...(isDropTarget && { outline: "2px dashed black" }) },
		}),
	});

	const rubSource = sourceItem ? rubSourceFor(sourceItem) : null;
	const tooltip = isDropTarget && rubSource ? rubOperationLabel(rubSource, target) : null;

	return (
		<Tooltip.Root open={tooltip !== null}>
			<Tooltip.Trigger render={droppable} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={styles.tooltip}>{tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const CommitMoveTarget: FC<{
	commitId: string;
	side: InsertSide;
	previousCommitId: string | undefined;
	nextCommitId: string | undefined;
}> = ({ commitId, side, previousCommitId, nextCommitId }) => {
	const sourceItem = useDraggedSourceItem();
	const isNoOp = (sourceCommitId: string): boolean =>
		sourceCommitId === commitId ||
		(side === "above" && previousCommitId === sourceCommitId) ||
		(side === "below" && nextCommitId === sourceCommitId);

	const [isDropTarget, dropRef] = useDroppable({
		canDrop: ({ source }) => {
			const sourceItem = parseDragData(source.data);
			return sourceItem?._tag === "Commit" && !isNoOp(sourceItem.commitId);
		},
		getData: (): OperationTarget => ({
			_tag: "CommitMove",
			relativeTo: { type: "commit", subject: commitId },
			side,
		}),
	});

	return (
		<div
			ref={dropRef}
			className={classes(
				styles.commitMoveTarget,
				sourceItem?._tag === "Commit" &&
					!isNoOp(sourceItem.commitId) &&
					styles.commitMoveTargetEnabled,
				isDropTarget && styles.commitMoveTargetActive,
			)}
		/>
	);
};

const StackMenuPopup: FC<{
	projectId: string;
	stackId: string;
}> = ({ projectId, stackId }) => {
	const unapplyStack = useMutation(unapplyStackMutationOptions);

	return (
		<Menu.Popup className={sharedStyles.menuPopup}>
			<Menu.Item className={sharedStyles.menuItem} disabled>
				Move to leftmost
			</Menu.Item>
			<Menu.Item className={sharedStyles.menuItem} disabled>
				Move to rightmost
			</Menu.Item>
			<Menu.Separator />
			<Menu.Item
				className={sharedStyles.menuItem}
				disabled={unapplyStack.isPending}
				onClick={() => {
					unapplyStack.mutate({ projectId, stackId });
				}}
			>
				{unapplyStack.isPending ? "Unapplying stack…" : "Unapply stack"}
			</Menu.Item>
		</Menu.Popup>
	);
};

const CommitC: FC<{
	projectId: string;
	commit: Commit;
	previousCommitId: string | undefined;
	nextCommitId: string | undefined;
	isHighlighted: boolean;
	isSelected: boolean;
	isAnyFileSelected: boolean;
	isFileSelected: (path: string) => boolean;
	toggleSelect: () => void;
	toggleFileSelect: (path: string) => void;
}> = ({
	projectId,
	commit,
	previousCommitId,
	nextCommitId,
	isHighlighted,
	isSelected,
	isAnyFileSelected,
	isFileSelected,
	toggleSelect,
	toggleFileSelect,
}) => {
	const expanded = isSelected || isAnyFileSelected;

	const changeUnit: ChangeUnit = { _tag: "commit", commitId: commit.id };

	return (
		<div className={sharedStyles.commit}>
			<CommitMoveTarget
				commitId={commit.id}
				side="above"
				previousCommitId={previousCommitId}
				nextCommitId={nextCommitId}
			/>
			<RubTarget
				target={changeUnit}
				render={
					<CommitRow
						projectId={projectId}
						commit={commit}
						isSelected={isSelected}
						isAnyFileSelected={isAnyFileSelected}
						isHighlighted={isHighlighted}
						toggleSelect={toggleSelect}
					/>
				}
			/>
			{expanded && (
				<div className={sharedStyles.commitDetails}>
					<Suspense fallback={<div>Loading changed details…</div>}>
						<CommitDetails
							projectId={projectId}
							commitId={commit.id}
							renderFile={(change) => (
								<div
									className={classes(
										sharedStyles.fileRow,
										isFileSelected(change.path) && sharedStyles.selected,
									)}
								>
									<DraggableFile
										change={change}
										changeUnit={changeUnit}
										render={
											<FileButton
												change={change}
												toggleSelect={() => toggleFileSelect(change.path)}
											/>
										}
									/>
								</div>
							)}
						/>
					</Suspense>
				</div>
			)}
			{nextCommitId === undefined && (
				<CommitMoveTarget
					commitId={commit.id}
					side="below"
					previousCommitId={previousCommitId}
					nextCommitId={nextCommitId}
				/>
			)}
		</div>
	);
};

const Changes: FC<{
	projectId: string;
	stackId: string | null;
	isFileSelected: (path: string) => boolean;
	toggleFileSelect: (path: string) => void;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	className?: string;
}> = ({ projectId, stackId, isFileSelected, toggleFileSelect, onDependencyHover, className }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));

	const changeUnit: ChangeUnit = { _tag: "changes", stackId };

	return (
		<RubTarget
			target={changeUnit}
			render={
				<div className={className}>
					{changes.length === 0 ? (
						<>No changes.</>
					) : (
						<ul className={sharedStyles.fileList}>
							{changes.map((change) => {
								const assignments = assignmentsByPath.get(change.path);
								const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);

								const dependencyCommitIds = hunkDependencyDiffs
									? dependencyCommitIdsForFile(hunkDependencyDiffs)
									: [];

								return (
									<li key={change.path}>
										<div
											className={classes(
												sharedStyles.fileRow,
												isFileSelected(change.path) && sharedStyles.selected,
											)}
										>
											<DraggableFile
												change={change}
												changeUnit={changeUnit}
												assignments={assignments}
												render={
													<FileButton
														change={change}
														toggleSelect={() => {
															toggleFileSelect(change.path);
														}}
													/>
												}
											/>
											{isNonEmptyArray(dependencyCommitIds) && (
												<DependencyIndicator
													projectId={projectId}
													commitIds={dependencyCommitIds}
													onHover={onDependencyHover}
												/>
											)}
										</div>
									</li>
								);
							})}
						</ul>
					)}
				</div>
			}
		/>
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

	const disabled = commitCreate.isPending || !relativeTo || diffSpecs.length === 0;

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
							if (response.pathsToRejectedChanges.length > 0)
								toastManager.add(
									rejectedChangesToastOptions({
										newCommit: response.newCommit,
										// Assertion is temporary until API response types have been fixed.
										pathsToRejectedChanges:
											response.pathsToRejectedChanges as Array<RejectedChange>,
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
			<button type="submit" disabled={disabled}>
				{commitCreate.isPending ? "Committing…" : "Commit"}
			</button>
		</form>
	);
};

const CommitMoveToBranchTarget: FC<
	{
		anchorRef: Array<number> | null;
		firstCommitId: string | undefined;
	} & useRender.ComponentProps<"div">
> = ({ anchorRef, firstCommitId, render, ...props }) => {
	const getOperationTarget = (sourceItem: SourceItem): OperationTarget | null => {
		if (anchorRef === null) return null;
		if (sourceItem._tag !== "Commit") return null;

		if (sourceItem.commitId === firstCommitId) return null;
		return {
			_tag: "CommitMove",
			relativeTo: {
				type: "referenceBytes",
				subject: anchorRef,
			},
			side: "below",
		};
	};

	const [isDropTarget, dropRef] = useDroppable({
		canDrop: ({ source }) => {
			const sourceItem = parseDragData(source.data);
			if (!sourceItem) return false;
			return !!getOperationTarget(sourceItem);
		},
		getData: ({ source }): OperationTarget | {} => {
			const sourceItem = parseDragData(source.data);
			if (!sourceItem) return {};
			return getOperationTarget(sourceItem) ?? {};
		},
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps(props, {
			style: { ...(isDropTarget && { outline: "2px dashed black" }) },
		}),
	});

	return (
		<Tooltip.Root open={isDropTarget}>
			<Tooltip.Trigger render={droppable} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={styles.tooltip}>Move here</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const StackC: FC<{
	projectId: string;
	stack: Stack;
	isCommitSelected: (commitId: string) => boolean;
	isCommitAnyFileSelected: (commitId: string) => boolean;
	isChangeUnitFileSelected: (changeUnit: ChangeUnit, path: string) => boolean;
	toggleCommitSelection: (commitId: string) => void;
	toggleChangeUnitFileSelection: (changeUnit: ChangeUnit, path: string) => void;
	highlightedCommitIds: Set<string>;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({
	projectId,
	stack,
	isCommitSelected,
	isCommitAnyFileSelected,
	isChangeUnitFileSelected,
	toggleCommitSelection,
	toggleChangeUnitFileSelection,
	highlightedCommitIds,
	onDependencyHover,
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

	const changesChangeUnit: ChangeUnit = { _tag: "changes", stackId };

	return (
		<div className={styles.stack}>
			<Menu.Root>
				<Menu.Trigger className={styles.stackMenu} style={{ lineHeight: 1 }}>
					𑁔
				</Menu.Trigger>
				<Menu.Portal>
					<Menu.Positioner align="end">
						<StackMenuPopup projectId={projectId} stackId={stackId} />
					</Menu.Positioner>
				</Menu.Portal>
			</Menu.Root>

			<div>
				<h3>Assigned changes</h3>
				<Changes
					projectId={projectId}
					stackId={stack.id}
					isFileSelected={(path) => isChangeUnitFileSelected(changesChangeUnit, path)}
					toggleFileSelect={(path) => {
						toggleChangeUnitFileSelection(changesChangeUnit, path);
					}}
					onDependencyHover={onDependencyHover}
					className={styles.assignedChanges}
				/>
				<CommitForm projectId={projectId} stack={stack} />
			</div>

			<ul className={styles.segments}>
				{stack.segments.map((segment) => {
					const branchName = segment.refName?.displayName ?? "Untitled";
					const anchorRef = segment.refName ? segment.refName.fullNameBytes : null;
					return (
						<li key={branchName}>
							<CommitMoveToBranchTarget
								anchorRef={anchorRef}
								firstCommitId={segment.commits[0]?.id}
								render={<h3>{branchName}</h3>}
							/>

							<h4>Commits</h4>
							<CommitsList commits={segment.commits}>
								{(commit, index) => {
									const changeUnit: ChangeUnit = {
										_tag: "commit",
										commitId: commit.id,
									};
									return (
										<CommitC
											projectId={projectId}
											commit={commit}
											previousCommitId={segment.commits[index - 1]?.id}
											nextCommitId={segment.commits[index + 1]?.id}
											isHighlighted={highlightedCommitIds.has(commit.id)}
											isSelected={isCommitSelected(commit.id)}
											isAnyFileSelected={isCommitAnyFileSelected(commit.id)}
											isFileSelected={(path) => isChangeUnitFileSelected(changeUnit, path)}
											toggleSelect={() => {
												toggleCommitSelection(commit.id);
											}}
											toggleFileSelect={(path) => {
												toggleChangeUnitFileSelection(changeUnit, path);
											}}
										/>
									);
								}}
							</CommitsList>
						</li>
					);
				})}
			</ul>
		</div>
	);
};

const ProjectPage: FC = () => {
	const { id: projectId } = projectRootRoute.useParams();

	const [highlightedCommitIds, setHighlightedCommitIds] = useState<Set<string>>(() => new Set());
	const [draggedSourceItem, setDraggedSourceItem] = useState<SourceItem | null>(null);

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());

	const project = projects.find((project) => project.id === projectId);

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const [_selection, select] = useLocalStorageState<Selection | null>(
		`project:${projectId}:workspace:selection`,
		{ defaultValue: null },
	);
	const commitStackIds = getStackIdsByCommitId(headInfo);
	const selection =
		(_selection ? normalizeSelection(_selection, commitStackIds) : null) ??
		getDefaultSelection({
			headInfo,
			changes: worktreeChanges.changes,
			assignments: worktreeChanges.assignments,
		});

	useMonitorDraggedSourceItem({ projectId, setDraggedSourceItem });

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	const baseId = commonBaseCommitId(headInfo);

	const isUnassignedFileSelected = (path: string): boolean =>
		selection?._tag === "changes" && selection.stackId === null && selection.path === path;
	const toggleUnassignedFileSelection = (path: string) => {
		select(isUnassignedFileSelected(path) ? null : { _tag: "changes", stackId: null, path });
	};
	const isCommitSelected = (stackId: string, commitId: string) =>
		selection?._tag === "commit" &&
		selection.stackId === stackId &&
		selection.commitId === commitId &&
		selection.path === undefined;
	const isCommitAnyFileSelected = (stackId: string, commitId: string) =>
		selection?._tag === "commit" &&
		selection.stackId === stackId &&
		selection.commitId === commitId &&
		selection.path !== undefined;
	const isChangeUnitFileSelected = (stackId: string, changeUnit: ChangeUnit, path: string) => {
		if (!selection) return false;
		if (selection._tag === "commit" && changeUnit._tag === "commit")
			return (
				selection.stackId === stackId &&
				selection.commitId === changeUnit.commitId &&
				selection.path === path
			);
		if (selection._tag === "changes" && changeUnit._tag === "changes")
			return selection.stackId === stackId && selection.path === path;
		return false;
	};
	const toggleCommitSelection = (stackId: string, commitId: string) => {
		select(isCommitSelected(stackId, commitId) ? null : { _tag: "commit", stackId, commitId });
	};
	const toggleChangeUnitFileSelection = (stackId: string, changeUnit: ChangeUnit, path: string) => {
		select(
			isChangeUnitFileSelected(stackId, changeUnit, path)
				? changeUnit._tag === "commit"
					? { _tag: "commit", stackId, commitId: changeUnit.commitId }
					: null
				: changeUnit._tag === "commit"
					? { _tag: "commit", stackId, commitId: changeUnit.commitId, path }
					: { _tag: "changes", stackId, path },
		);
	};

	const highlightCommits = (commitIds: Array<string> | null) => {
		setHighlightedCommitIds(commitIds ? new Set(commitIds) : new Set());
	};

	return (
		<DraggedSourceItemContext.Provider value={draggedSourceItem}>
			<ProjectPanelLayout
				projectId={projectId}
				preview={
					selection && (
						<Suspense fallback={<div>Loading diff…</div>}>
							<Preview
								projectId={projectId}
								selection={selection}
								onDependencyHover={highlightCommits}
							/>
						</Suspense>
					)
				}
			>
				<>
					<div className={sharedStyles.lanes}>
						<div className={sharedStyles.commitsLane}>
							<h3>Unassigned changes</h3>
							<Changes
								projectId={project.id}
								stackId={null}
								isFileSelected={isUnassignedFileSelected}
								toggleFileSelect={toggleUnassignedFileSelection}
								onDependencyHover={highlightCommits}
								className={styles.unassignedChanges}
							/>
						</div>

						{headInfo.stacks.map((stack) => {
							// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
							const stackId = stack.id!;

							return (
								<div key={stack.id} className={sharedStyles.commitsLane}>
									<StackC
										key={stack.id}
										projectId={project.id}
										stack={stack}
										isCommitSelected={(commitId) => isCommitSelected(stackId, commitId)}
										isCommitAnyFileSelected={(commitId) =>
											isCommitAnyFileSelected(stackId, commitId)
										}
										isChangeUnitFileSelected={(changeUnit, path) =>
											isChangeUnitFileSelected(stackId, changeUnit, path)
										}
										toggleCommitSelection={(commitId) => {
											toggleCommitSelection(stackId, commitId);
										}}
										toggleChangeUnitFileSelection={(changeUnit, path) => {
											toggleChangeUnitFileSelection(stackId, changeUnit, path);
										}}
										highlightedCommitIds={highlightedCommitIds}
										onDependencyHover={highlightCommits}
									/>
								</div>
							);
						})}
					</div>

					{baseId !== undefined && <div>{shortCommitId(baseId)} (common base commit)</div>}
				</>
			</ProjectPanelLayout>
		</DraggedSourceItemContext.Provider>
	);
};

export const projectIndexRoute = createRoute({
	getParentRoute: () => projectRootRoute,
	path: "/",
	component: ProjectPage,
});
