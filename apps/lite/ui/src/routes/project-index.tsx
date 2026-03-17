import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { Menu, mergeProps, Popover, Tooltip, useRender } from "@base-ui/react";
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
} from "@gitbutler/but-sdk";
import { Array, Match } from "effect";
import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import { createContext, FC, Suspense, useContext, useEffect, useState } from "react";
import styles from "./project-index.module.css";
import sharedStyles from "./project-shared.module.css";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import { useDroppable } from "#ui/hooks/useDroppable.ts";
import { useLocalStorageState } from "#ui/hooks/useLocalStorageState.ts";
import {
	CommitDetails,
	CommitRow,
	CommitsList,
	type DragData,
	FileButton,
	FileDiff,
	Hunk,
	type SourceItem,
} from "#ui/routes/project-shared.tsx";
import {
	commitMoveMutationOptions,
	commitMutationOptions,
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
import { rubOperationLabel, type RubSource } from "#ui/rub.ts";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { createDiffSpec } from "#ui/DiffSpec.ts";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";

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

const commonBaseCommitId = (headInfo: RefInfo): string | undefined => {
	const bases = headInfo.stacks
		.map((stack) => stack.base)
		.filter((base): base is string => base !== null);
	const first = bases[0];
	if (first === undefined) return undefined;
	return bases.every((base) => base === first) ? first : undefined;
};

const rubSourceFor = (item: SourceItem): RubSource | null => {
	switch (item._tag) {
		case "Commit":
			return { _tag: "Commit", source: { commitId: item.commitId } };
		case "TreeChange":
			return { _tag: "TreeChange", source: item.source };
	}
};

const DraggedSourceItemContext = createContext<SourceItem | null>(null);

const parseDragData = (data: unknown): SourceItem | null => {
	if (typeof data !== "object" || data === null || !("sourceItem" in data)) return null;
	return (data as DragData).sourceItem;
};

const useDraggedSourceItem = (): SourceItem | null => useContext(DraggedSourceItemContext);

type OperationTarget =
	| {
			_tag: "Rub";
			target: ChangeUnit;
	  }
	| {
			_tag: "CommitMove";
			anchorCommitId: string;
			side: InsertSide;
			previousCommitId: string | undefined;
			nextCommitId: string | undefined;
	  }
	| {
			_tag: "CommitMoveToBranch";
			anchorRef: Array<number> | null;
			firstCommitId: string | undefined;
	  };

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
	const rubMutation = useMutation(rubMutationOptions);
	const commitMove = useMutation(commitMoveMutationOptions);

	return (sourceItem: SourceItem, operationTarget: OperationTarget): void => {
		Match.value(operationTarget).pipe(
			Match.tag("Rub", (operationTarget) => {
				const rubSource = rubSourceFor(sourceItem);
				if (!rubSource) return;
				rubMutation.mutate({
					projectId,
					source: rubSource,
					target: operationTarget.target,
				});
			}),
			Match.tag("CommitMove", (operationTarget) => {
				if (sourceItem._tag !== "Commit") return;
				commitMove.mutate({
					projectId,
					subjectCommitId: sourceItem.commitId,
					relativeTo: { type: "commit", subject: operationTarget.anchorCommitId },
					side: operationTarget.side,
				});
			}),
			Match.tag("CommitMoveToBranch", (operationTarget) => {
				if (sourceItem._tag !== "Commit" || operationTarget.anchorRef === null) return;
				commitMove.mutate({
					projectId,
					subjectCommitId: sourceItem.commitId,
					relativeTo: { type: "referenceBytes", subject: operationTarget.anchorRef },
					side: "below",
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

const hunkContainsHunk = (a: DiffHunk, b: DiffHunk): boolean =>
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
		preview: <div className={sharedStyles.dragPreview}>{change.path}</div>,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && sharedStyles.dragging),
		}),
	});
};

const SelectedChangesFileDiff: FC<{
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

const SelectedCommitFileDiff: FC<{
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

const SelectedCommitDiff: FC<{
	projectId: string;
	commitId: string;
}> = ({ projectId, commitId }) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	if (data.changes.length === 0) return null;

	return (
		<ul className={sharedStyles.hunks}>
			{data.changes.map((change) => (
				<li key={change.path}>
					<h5>{change.path}</h5>
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
	);
};

const RubTarget: FC<{
	target: ChangeUnit;
	children: React.ReactElement;
}> = ({ target, children }) => {
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

	const rubSource = sourceItem ? rubSourceFor(sourceItem) : null;
	const tooltip = isDropTarget && rubSource ? rubOperationLabel(rubSource, target) : null;

	return (
		<Tooltip.Root open={tooltip !== null}>
			<Tooltip.Trigger
				ref={dropRef}
				render={children}
				style={{ ...(isDropTarget && { outline: "2px dashed" }) }}
			/>
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
			anchorCommitId: commitId,
			side,
			previousCommitId,
			nextCommitId,
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
			<RubTarget target={changeUnit}>
				<CommitRow
					projectId={projectId}
					commit={commit}
					isSelected={isSelected}
					isAnyFileSelected={isAnyFileSelected}
					isHighlighted={isHighlighted}
					toggleSelect={toggleSelect}
				/>
			</RubTarget>
			{expanded && (
				<div className={sharedStyles.commitDetails}>
					<Suspense fallback={<div>Loading changed details…</div>}>
						<CommitDetails
							projectId={projectId}
							commitId={commit.id}
							renderFile={(change) => (
								<div className={sharedStyles.fileRow}>
									<DraggableFile
										change={change}
										changeUnit={changeUnit}
										render={
											<FileButton
												change={change}
												isSelected={isFileSelected(change.path)}
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
		<RubTarget target={changeUnit}>
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
									<div className={sharedStyles.fileRow}>
										<DraggableFile
											change={change}
											changeUnit={changeUnit}
											assignments={assignments}
											render={
												<FileButton
													change={change}
													isSelected={isFileSelected(change.path)}
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
		</RubTarget>
	);
};

const CommitForm: FC<{
	projectId: string;
	stack: Stack;
}> = ({ projectId, stack }) => {
	const [message, setMessage] = useLocalStorageState(
		// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
		`project:${projectId}:commitMessage:${stack.id!}`,
		"",
	);
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const relativeTo = stackRelativeTo(stack);
	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stack.id);
	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));
	const diffSpecs = assignedChangesDiffSpecs(changes, assignmentsByPath);

	const commit = useMutation(commitMutationOptions);

	const disabled =
		commit.isPending || !relativeTo || diffSpecs.length === 0 || message.trim().length === 0;

	return (
		<form
			className={styles.commitForm}
			onSubmit={(event) => {
				event.preventDefault();
				if (disabled) return;
				commit.mutate(
					{
						projectId,
						relativeTo,
						side: "below",
						changes: diffSpecs,
						message: message.trim(),
					},
					{
						onSuccess: () => {
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
				{commit.isPending ? "Committing…" : "Commit"}
			</button>
		</form>
	);
};

type UnassignedLaneSelection = {
	path: string;
};

const UnassignedLane: FC<{
	projectId: string;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, onDependencyHover }) => {
	const [selection, select] = useLocalStorageState<UnassignedLaneSelection | null>(
		`project:${projectId}:unassignedChangesLaneSelection`,
		null,
	);

	const isFileSelected = (path: string): boolean => selection?.path === path;

	const toggleFileSelection = (path: string): UnassignedLaneSelection | null =>
		isFileSelected(path) ? null : { path };

	return (
		<div className={sharedStyles.laneColumns}>
			<div className={sharedStyles.laneMainColumn}>
				<div>
					<h3>Unassigned changes</h3>
					<Changes
						projectId={projectId}
						stackId={null}
						isFileSelected={isFileSelected}
						toggleFileSelect={(path) => {
							select(toggleFileSelection(path));
						}}
						onDependencyHover={onDependencyHover}
						className={styles.unassignedChanges}
					/>
				</div>
			</div>

			{selection !== null && (
				<div className={sharedStyles.laneDiffColumn}>
					<Suspense fallback={<div>Loading diff…</div>}>
						<SelectedChangesFileDiff
							projectId={projectId}
							stackId={null}
							path={selection.path}
							onDependencyHover={onDependencyHover}
						/>
					</Suspense>
				</div>
			)}
		</div>
	);
};

type StackLaneSelection =
	| {
			_tag: "commit";
			commitId: string;
			path?: string;
	  }
	| {
			_tag: "changes";
			path: string;
	  };

const CommitMoveToBranchTarget: FC<{
	anchorRef: Array<number> | null;
	firstCommitId: string | undefined;
	children: React.ReactElement;
}> = ({ anchorRef, firstCommitId, children }) => {
	const [isDropTarget, dropRef] = useDroppable({
		canDrop: ({ source }) => {
			const sourceItem = parseDragData(source.data);
			return sourceItem?._tag === "Commit" && firstCommitId !== sourceItem.commitId;
		},
		disabled: anchorRef === null,
		getData: (): OperationTarget => ({
			_tag: "CommitMoveToBranch",
			anchorRef,
			firstCommitId,
		}),
	});

	return (
		<Tooltip.Root open={isDropTarget}>
			<Tooltip.Trigger
				ref={dropRef}
				render={children}
				style={{ ...(isDropTarget && { outline: "2px dashed" }) }}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={styles.tooltip}>Move here</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const StackLane: FC<{
	projectId: string;
	stack: Stack;
	highlightedCommitIds: Set<string>;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, stack, highlightedCommitIds, onDependencyHover }) => {
	// From Caleb:
	// > There shouldn't be a way within GitButler to end up with a stack without a
	//   StackId. Users can disrupt our matching against our metadata by playing
	//   with references, but we currently also try to patch it up at certain points
	//   so it probably isn't too common.
	// For now we'll treat this as non-nullable until we identify cases where it
	// could genuinely be null (assuming backend correctness).
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [tag:stack-id-required]
	const stackId = stack.id!;

	const [selection, select] = useLocalStorageState<StackLaneSelection | null>(
		`project:${projectId}:stackLaneSelection:${stackId}`,
		null,
	);

	const isCommitSelected = (commitId: string) =>
		selection?._tag === "commit" && selection.commitId === commitId && selection.path === undefined;

	const isCommitAnyFileSelected = (commitId: string) =>
		selection?._tag === "commit" && selection.commitId === commitId && selection.path !== undefined;

	const isChangeUnitFileSelected = (changeUnit: ChangeUnit, path: string) => {
		if (!selection) return false;
		if (selection._tag === "commit" && changeUnit._tag === "commit")
			return selection.commitId === changeUnit.commitId && selection.path === path;
		if (selection._tag === "changes" && changeUnit._tag === "changes")
			return selection.path === path;
		return false;
	};

	const toggleCommitSelection = (commitId: string): StackLaneSelection | null =>
		isCommitSelected(commitId) ? null : { _tag: "commit", commitId };

	const toggleChangeUnitFileSelection = (
		changeUnit: ChangeUnit,
		path: string,
	): StackLaneSelection | null =>
		isChangeUnitFileSelected(changeUnit, path)
			? changeUnit._tag === "commit"
				? { _tag: "commit", commitId: changeUnit.commitId }
				: null
			: changeUnit._tag === "commit"
				? { _tag: "commit", commitId: changeUnit.commitId, path }
				: { _tag: "changes", path };

	const changesChangeUnit: ChangeUnit = { _tag: "changes", stackId };

	return (
		<div className={sharedStyles.laneColumns}>
			<div className={sharedStyles.laneMainColumn}>
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
							select(toggleChangeUnitFileSelection(changesChangeUnit, path));
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
								>
									<h3>{branchName}</h3>
								</CommitMoveToBranchTarget>

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
													select(toggleCommitSelection(commit.id));
												}}
												toggleFileSelect={(path) => {
													select(toggleChangeUnitFileSelection(changeUnit, path));
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

			{selection !== null && (
				<div className={sharedStyles.laneDiffColumn}>
					<Suspense fallback={<div>Loading diff…</div>}>
						{Match.value(selection).pipe(
							Match.tag("changes", ({ path }) => (
								<SelectedChangesFileDiff
									projectId={projectId}
									stackId={stackId}
									path={path}
									onDependencyHover={onDependencyHover}
								/>
							)),
							Match.tag("commit", ({ commitId, path }) =>
								path !== undefined ? (
									<SelectedCommitFileDiff projectId={projectId} commitId={commitId} path={path} />
								) : (
									<SelectedCommitDiff projectId={projectId} commitId={commitId} />
								),
							),
							Match.exhaustive,
						)}
					</Suspense>
				</div>
			)}
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

	useMonitorDraggedSourceItem({ projectId, setDraggedSourceItem });

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	const baseId = commonBaseCommitId(headInfo);

	const highlightCommits = (commitIds: Array<string> | null) => {
		setHighlightedCommitIds(commitIds ? new Set(commitIds) : new Set());
	};

	return (
		<DraggedSourceItemContext.Provider value={draggedSourceItem}>
			<h2>{project.title} workspace</h2>

			<div className={sharedStyles.lanes}>
				<UnassignedLane projectId={project.id} onDependencyHover={highlightCommits} />

				{headInfo.stacks.map((stack) => (
					<StackLane
						key={stack.id}
						projectId={project.id}
						stack={stack}
						highlightedCommitIds={highlightedCommitIds}
						onDependencyHover={highlightCommits}
					/>
				))}
			</div>

			{baseId !== undefined && <>{shortCommitId(baseId)} (common base commit)</>}
		</DraggedSourceItemContext.Provider>
	);
};

export const projectIndexRoute = createRoute({
	getParentRoute: () => projectRootRoute,
	path: "/",
	component: ProjectPage,
});
