import useLocalStorageState from "use-local-storage-state";
import { monitorForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { Menu, mergeProps, Popover, Toast, Tooltip, useRender } from "@base-ui/react";
import {
	Commit,
	DiffHunk,
	HunkAssignment,
	InsertSide,
	TreeChange,
	DiffSpec,
	Stack,
	HunkDependencies,
	HunkHeader,
} from "@gitbutler/but-sdk";
import { createRoute } from "@tanstack/react-router";
import { Array, Match, pipe } from "effect";
import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { FC, Suspense, useEffect, useState } from "react";
import styles from "./index.module.css";
import sharedStyles from "../shared.module.css";
import { DependencyIcon, MenuTriggerIcon } from "#ui/components/icons.tsx";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import { useDroppable } from "#ui/hooks/useDroppable.ts";
import { type Operation, type RubOperation, useRunOperation } from "#ui/Operation.ts";
import { ProjectPreviewLayout } from "#ui/routes/project/$id/ProjectPreviewLayout.tsx";
import {
	CommitDetails,
	CommitLabel,
	CommitRow,
	CommitsList,
	type DragData,
	DragPreview,
	DraggableBranch,
	FileButton,
	FileDiff,
	Hunk,
	type SourceItem,
} from "#ui/routes/project/$id/shared.tsx";
import { commitCreateMutationOptions, unapplyStackMutationOptions } from "#ui/api/mutations.ts";
import {
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { RejectedChange, rejectedChangesToastOptions } from "#ui/components/RejectedChanges.tsx";
import { type RubSource } from "#ui/api/rub.ts";
import {
	getBranchNameByCommitId,
	getBranchRefsByStackId,
	getCommonBaseCommitId,
	getSegmentBranchRef,
	getStackIdsByCommitId,
} from "#ui/domain/RefInfo.ts";
import { stackRelativeTo } from "#ui/domain/Stack.ts";
import { getDefaultSelection, normalizeSelection, type Selection } from "./WorkspaceSelection.ts";
import { projectRoute } from "#ui/routes/project/$id/route.tsx";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";

const rubSourceFor = (item: SourceItem): RubSource | null =>
	Match.value(item).pipe(
		Match.tag("Branch", (): RubSource | null => null),
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

type RubOperationLabel = "Amend" | "Uncommit" | "Assign" | "Unassign" | "Squash";

/**
 * | SOURCE ↓ / TARGET →    | Unassigned changes | Assigned changes | Commit |
 * | ---------------------- | ------------------ | ---------------- | ------ |
 * | File/hunk from changes | Unassign           | Assign           | Amend  |
 * | File/hunk from commit  | Uncommit           | Uncommit         | Amend  |
 * | Commit                 | Uncommit           | Uncommit         | Squash |
 *
 * Note this is currently different from the CLI's definition of "rubbing" which
 * includes move operations.
 * https://linear.app/gitbutler/issue/GB-1160/what-should-rubbing-a-branch-into-another-branch-do#comment-db2abdb7
 */
const rubOperationLabel = ({ source, target }: RubOperation): RubOperationLabel | null =>
	Match.value(source).pipe(
		Match.withReturnType<RubOperationLabel | null>(),
		Match.tag("TreeChange", ({ source }) =>
			Match.value(source.parent).pipe(
				Match.withReturnType<RubOperationLabel | null>(),
				Match.tag("Changes", (source) =>
					Match.value(target).pipe(
						Match.withReturnType<RubOperationLabel | null>(),
						Match.tag("Changes", (target) => {
							if (source.stackId === target.stackId) return null;
							return target.stackId === null ? "Unassign" : "Assign";
						}),
						Match.tag("Commit", () => "Amend"),
						Match.exhaustive,
					),
				),
				Match.tag("Commit", (source) =>
					Match.value(target).pipe(
						Match.withReturnType<RubOperationLabel | null>(),
						Match.tag("Changes", () => "Uncommit"),
						Match.tag("Commit", (target) => {
							if (source.commitId === target.commitId) return null;
							return "Amend";
						}),
						Match.exhaustive,
					),
				),
				Match.exhaustive,
			),
		),
		Match.tag("Commit", ({ source }) =>
			Match.value(target).pipe(
				Match.withReturnType<RubOperationLabel | null>(),
				Match.tag("Changes", () => "Uncommit"),
				Match.tag("Commit", (target) => {
					if (source.commitId === target.commitId) return null;
					return "Squash";
				}),
				Match.exhaustive,
			),
		),
		Match.exhaustive,
	);

// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
const decodeRefName = (fullNameBytes: Array<number>): string =>
	new TextDecoder().decode(Uint8Array.from(fullNameBytes));

const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

type HunkDependencyDiff = HunkDependencies["diffs"][number];

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
		<Popover.Root>
			<Popover.Trigger openOnHover render={trigger} />
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

const parseDragData = (data: unknown): SourceItem | null => {
	if (typeof data !== "object" || data === null || !("sourceItem" in data)) return null;
	return (data as DragData).sourceItem;
};

const getRubOperation = ({
	sourceItem,
	target,
}: {
	sourceItem: SourceItem;
	target: ChangeUnit;
}): RubOperation | null => {
	const rubSource = rubSourceFor(sourceItem);
	if (!rubSource) return null;
	const rubOperation: RubOperation = {
		source: rubSource,
		target,
	};
	if (rubOperationLabel(rubOperation) === null) return null;
	return rubOperation;
};

const parseDropTargetData = (data: unknown): Operation | null => {
	if (typeof data !== "object" || data === null || !("_tag" in data)) return null;
	return data as Operation;
};

const useMonitorDraggedSourceItem = ({ projectId }: { projectId: string }): void => {
	const runOperation = useRunOperation(projectId);

	useEffect(
		() =>
			monitorForElements({
				canMonitor: ({ source }) => parseDragData(source.data) !== null,
				onDrop: ({ location }) => {
					const operation = location.current.dropTargets
						.map((dropTarget) => parseDropTargetData(dropTarget.data))
						.find((target) => target);

					if (!operation) return;

					runOperation(operation);
				},
			}),
		[runOperation],
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
						changeUnit={{ _tag: "Changes", stackId }}
						change={change}
						hunk={hunk}
						headerStart={
							isNonEmptyArray(dependencyCommitIds) && (
								<DependencyIndicator
									projectId={projectId}
									commitIds={dependencyCommitIds}
									onHover={onDependencyHover}
								>
									<DependencyIcon />
								</DependencyIndicator>
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
				<Hunk patch={patch} changeUnit={{ _tag: "Commit", commitId }} change={change} hunk={hunk} />
			)}
		/>
	);
};

const ShowBranch: FC<{
	projectId: string;
	branch: string;
	branchName: string;
}> = ({ projectId, branch, branchName }) => {
	const { data } = useSuspenseQuery(branchDiffQueryOptions({ projectId, branch }));

	return (
		<>
			<h3>{branchName}</h3>
			{data.changes.length === 0 ? (
				<div>No file changes.</div>
			) : (
				<ul>
					{data.changes.map((change) => (
						<li key={change.path}>
							<h4>{change.path}</h4>
							<FileDiff
								projectId={projectId}
								change={change}
								renderHunk={(hunk, patch) => (
									<Hunk
										patch={patch}
										changeUnit={{ _tag: "Changes", stackId: null }}
										change={change}
										hunk={hunk}
									/>
								)}
							/>
						</li>
					))}
				</ul>
			)}
		</>
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
			<ul>
				{data.changes.map((change) => (
					<li key={change.path}>
						<h4>{change.path}</h4>
						<FileDiff
							projectId={projectId}
							change={change}
							renderHunk={(hunk, patch) => (
								<Hunk
									patch={patch}
									changeUnit={{ _tag: "Commit", commitId }}
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

const Preview: FC<{
	projectId: string;
	selection: Selection;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, selection, onDependencyHover }) =>
	Match.value(selection).pipe(
		Match.tag("Branch", ({ branchName, branchRef }) => (
			<ShowBranch projectId={projectId} branch={branchRef} branchName={branchName} />
		)),
		Match.tag("ChangesFile", ({ stackId, path }) => (
			<ChangesFileDiff
				projectId={projectId}
				stackId={stackId}
				path={path}
				onDependencyHover={onDependencyHover}
			/>
		)),
		Match.tag("Commit", ({ commitId }) => <ShowCommit projectId={projectId} commitId={commitId} />),
		Match.tag("CommitFile", ({ commitId, path }) => (
			<CommitFileDiff projectId={projectId} commitId={commitId} path={path} />
		)),
		Match.exhaustive,
	);

const ChangesTarget: FC<
	{
		stackId: string | null;
	} & useRender.ComponentProps<"div">
> = ({ stackId, render, ...props }) => {
	const getOperation = (sourceItem: SourceItem): Operation | null => {
		const rubOperation = getRubOperation({ sourceItem, target: { _tag: "Changes", stackId } });
		if (!rubOperation) return null;
		return { _tag: "Rub", ...rubOperation };
	};

	const [operation, dropRef] = useDroppable(({ source }) => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem) return null;
		return getOperation(sourceItem);
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps(props, {
			className: classes(operation && styles.dragOver),
		}),
	});

	const tooltip = operation && operation._tag === "Rub" ? rubOperationLabel(operation) : null;

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

const CommitTarget: FC<
	{
		commitId: string;
		previousCommitId: string | undefined;
		nextCommitId: string | undefined;
	} & useRender.ComponentProps<"div">
> = ({ commitId, previousCommitId, nextCommitId, render, ...props }) => {
	const isNoOpCommitMove = (sourceCommitId: string, side: InsertSide): boolean =>
		sourceCommitId === commitId ||
		(side === "above" && previousCommitId === sourceCommitId) ||
		(side === "below" && nextCommitId === sourceCommitId);

	const [operation, dropRef] = useDroppable(({ source, input, element }) => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem) return null;

		const rubOperation = getRubOperation({ sourceItem, target: { _tag: "Commit", commitId } });

		const instruction = extractInstruction(
			attachInstruction(
				{ sourceItem },
				{
					input,
					element,
					operations: {
						"reorder-before":
							sourceItem._tag === "Commit" && !isNoOpCommitMove(sourceItem.commitId, "above")
								? "available"
								: "not-available",
						"reorder-after":
							sourceItem._tag === "Commit" && !isNoOpCommitMove(sourceItem.commitId, "below")
								? "available"
								: "not-available",
						combine: rubOperation ? "available" : "not-available",
					},
				},
			),
		);

		if (!instruction) return null;

		return Match.value(instruction.operation).pipe(
			Match.when("combine", (): Operation | null =>
				rubOperation ? { _tag: "Rub", ...rubOperation } : null,
			),
			Match.when("reorder-before", (): Operation | null =>
				sourceItem._tag === "Commit"
					? {
							_tag: "CommitMove",
							subjectCommitId: sourceItem.commitId,
							relativeTo: { type: "commit", subject: commitId },
							side: "above",
						}
					: null,
			),
			Match.when("reorder-after", (): Operation | null =>
				sourceItem._tag === "Commit"
					? {
							_tag: "CommitMove",
							subjectCommitId: sourceItem.commitId,
							relativeTo: { type: "commit", subject: commitId },
							side: "below",
						}
					: null,
			),
			Match.exhaustive,
		);
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation?._tag === "Rub" && styles.dragOver),
		}),
	});

	const tooltip = operation
		? Match.value(operation).pipe(
				Match.tag("Rub", (operation) => rubOperationLabel(operation)),
				Match.tag("CommitMove", () => null),
				Match.orElse(() => null),
			)
		: null;

	return (
		<div className={styles.commit}>
			<Tooltip.Root open={tooltip !== null}>
				<Tooltip.Trigger render={droppable} />
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={8}>
						<Tooltip.Popup className={styles.tooltip}>{tooltip}</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
			{operation?._tag === "CommitMove" && (
				<div
					className={classes(
						styles.commitMoveTarget,
						pipe(
							operation.side,
							Match.value,
							Match.when("above", () => styles.commitMoveTargetAbove),
							Match.when("below", () => styles.commitMoveTargetBelow),
							Match.exhaustive,
						),
					)}
				/>
			)}
		</div>
	);
};

const CommitC: FC<{
	projectId: string;
	commit: Commit;
	previousCommitId: string | undefined;
	nextCommitId: string | undefined;
	isHighlighted: boolean;
	isSelected: boolean;
	isEditingMessage: boolean;
	isSelectedWithin: boolean;
	isFileSelected: (path: string) => boolean;
	toggleExpand: () => Promise<void> | void;
	toggleSelect: () => void;
	toggleEditingMessage: () => void;
	toggleFileSelect: (path: string) => void;
}> = ({
	projectId,
	commit,
	previousCommitId,
	nextCommitId,
	isHighlighted,
	isSelected,
	isEditingMessage,
	isSelectedWithin,
	isFileSelected,
	toggleExpand,
	toggleSelect,
	toggleEditingMessage,
	toggleFileSelect,
}) => (
	<CommitTarget
		commitId={commit.id}
		previousCommitId={previousCommitId}
		nextCommitId={nextCommitId}
	>
		<CommitRow
			projectId={projectId}
			commit={commit}
			isSelected={isSelected}
			isEditingMessage={isEditingMessage}
			isSelectedWithin={isSelectedWithin}
			isHighlighted={isHighlighted}
			toggleExpand={toggleExpand}
			toggleSelect={toggleSelect}
			toggleEditingMessage={toggleEditingMessage}
		/>
		{isSelectedWithin && (
			<div className={sharedStyles.commitDetails}>
				<Suspense fallback={<div>Loading changed details…</div>}>
					<CommitDetails
						projectId={projectId}
						commitId={commit.id}
						renderFile={(change) => (
							<DraggableFile
								change={change}
								changeUnit={{ _tag: "Commit", commitId: commit.id }}
								render={
									<div
										className={classes(
											sharedStyles.row,
											sharedStyles.fileRow,
											isFileSelected(change.path) && sharedStyles.selected,
										)}
									>
										<FileButton
											change={change}
											toggleSelect={() => toggleFileSelect(change.path)}
										/>
									</div>
								}
							/>
						)}
					/>
				</Suspense>
			</div>
		)}
	</CommitTarget>
);

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

	return (
		<ChangesTarget stackId={stackId} className={className}>
			{changes.length === 0 ? (
				<>No changes.</>
			) : (
				<ul>
					{changes.map((change) => {
						const assignments = assignmentsByPath.get(change.path);
						const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);

						const dependencyCommitIds = hunkDependencyDiffs
							? dependencyCommitIdsForFile(hunkDependencyDiffs)
							: [];

						return (
							<li key={change.path}>
								<DraggableFile
									change={change}
									changeUnit={{ _tag: "Changes", stackId }}
									assignments={assignments}
									render={
										<div
											className={classes(
												sharedStyles.row,
												sharedStyles.fileRow,
												isFileSelected(change.path) && sharedStyles.selected,
											)}
										>
											<FileButton
												change={change}
												toggleSelect={() => {
													toggleFileSelect(change.path);
												}}
											/>
											{isNonEmptyArray(dependencyCommitIds) && (
												<DependencyIndicator
													projectId={projectId}
													commitIds={dependencyCommitIds}
													onHover={onDependencyHover}
													className={sharedStyles.rowAction}
												>
													<DependencyIcon />
												</DependencyIndicator>
											)}
										</div>
									}
								/>
							</li>
						);
					})}
				</ul>
			)}
		</ChangesTarget>
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
			<button type="submit" disabled={disabled} className={sharedStyles.button}>
				{commitCreate.isPending ? "Committing…" : "Commit"}
			</button>
		</form>
	);
};

const BranchTarget: FC<
	{
		anchorRef: Array<number> | null;
		firstCommitId: string | undefined;
	} & useRender.ComponentProps<"div">
> = ({ anchorRef, firstCommitId, render, ...props }) => {
	const getOperation = (sourceItem: SourceItem): Operation | null =>
		Match.value(sourceItem).pipe(
			Match.tag("Branch", (source): Operation | null => {
				if (anchorRef === null || decodeRefName(anchorRef) === decodeRefName(source.anchorRef))
					return null;
				return {
					_tag: "MoveBranch",
					subjectBranch: decodeRefName(source.anchorRef),
					targetBranch: decodeRefName(anchorRef),
				};
			}),
			Match.tag("Commit", ({ commitId }): Operation | null => {
				if (anchorRef === null || commitId === firstCommitId) return null;
				return {
					_tag: "CommitMove",
					subjectCommitId: commitId,
					relativeTo: {
						type: "referenceBytes",
						subject: anchorRef,
					},
					side: "below",
				};
			}),
			Match.orElse(() => null),
		);

	const [operation, dropRef] = useDroppable<Operation>(({ source }) => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem) return null;
		return getOperation(sourceItem);
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps(props, {
			className: classes(operation && styles.dragOver),
		}),
	});

	const tooltip = operation
		? Match.value(operation).pipe(
				Match.tag("MoveBranch", () => "Stack branch onto here"),
				Match.tag("CommitMove", () => "Move commit here"),
				Match.orElse(() => null),
			)
		: null;

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

const TearOffBranchTarget: FC<useRender.ComponentProps<"div">> = ({ render, ...props }) => {
	const getOperation = (sourceItem: SourceItem): Operation | null => {
		if (sourceItem._tag !== "Branch") return null;
		return {
			_tag: "TearOffBranch",
			subjectBranch: decodeRefName(sourceItem.anchorRef),
		};
	};

	const [operation, dropRef] = useDroppable<Operation>(({ source }) => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem) return null;
		return getOperation(sourceItem);
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps(props, {
			className: classes(operation && styles.dragOver),
		}),
	});

	return (
		<Tooltip.Root open={operation !== null}>
			<Tooltip.Trigger render={droppable} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={styles.tooltip}>Tear off branch</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const StackC: FC<{
	projectId: string;
	stack: Stack;
	isBranchSelected: (stackId: string, branchRef: string) => boolean;
	toggleBranchSelection: (stackId: string, branchName: string, branchRef: string) => void;
	isCommitSelected: (commitId: string) => boolean;
	isCommitEditing: (commitId: string) => boolean;
	isCommitSelectedWithin: (commitId: string) => boolean;
	isChangeUnitFileSelected: (changeUnit: ChangeUnit, path: string) => boolean;
	toggleCommitExpanded: (commitId: string) => Promise<void> | void;
	toggleCommitSelection: (commitId: string) => void;
	toggleEditingMessage: (commitId: string) => void;
	toggleChangeUnitFileSelection: (changeUnit: ChangeUnit, path: string) => void;
	highlightedCommitIds: Set<string>;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({
	projectId,
	stack,
	isBranchSelected,
	toggleBranchSelection,
	isCommitSelected,
	isCommitEditing,
	isCommitSelectedWithin,
	isChangeUnitFileSelected,
	toggleCommitExpanded,
	toggleCommitSelection,
	toggleEditingMessage,
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

	const changesChangeUnit: ChangeUnit = { _tag: "Changes", stackId };

	return (
		<div className={styles.stack}>
			<div>
				<div className={styles.stackHeader}>
					<h3>Assigned changes</h3>
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
					const branchRef = getSegmentBranchRef(segment);
					const anchorRef = segment.refName ? segment.refName.fullNameBytes : null;
					return (
						<li key={branchName}>
							<BranchTarget
								anchorRef={anchorRef}
								firstCommitId={segment.commits[0]?.id}
								render={
									<DraggableBranch
										anchorRef={anchorRef}
										label={branchName}
										render={
											branchRef !== null ? (
												<button
													type="button"
													className={classes(
														styles.branchButton,
														isBranchSelected(stackId, branchRef) && sharedStyles.selected,
													)}
													onClick={() => {
														toggleBranchSelection(stackId, branchName, branchRef);
													}}
												>
													{branchName}
												</button>
											) : (
												<div>{branchName}</div>
											)
										}
									/>
								}
							/>

							<CommitsList commits={segment.commits}>
								{(commit, index) => {
									const changeUnit: ChangeUnit = {
										_tag: "Commit",
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
											isEditingMessage={isCommitEditing(commit.id)}
											isSelectedWithin={isCommitSelectedWithin(commit.id)}
											isFileSelected={(path) => isChangeUnitFileSelected(changeUnit, path)}
											toggleExpand={() => toggleCommitExpanded(commit.id)}
											toggleSelect={() => {
												toggleCommitSelection(commit.id);
											}}
											toggleEditingMessage={() => {
												toggleEditingMessage(commit.id);
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
	const { id: projectId } = projectRoute.useParams();

	const [highlightedCommitIds, setHighlightedCommitIds] = useState<Set<string>>(() => new Set());

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());

	const project = projects.find((project) => project.id === projectId);

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const queryClient = useQueryClient();

	const [_selection, select] = useLocalStorageState<Selection | null>(
		`project:${projectId}:workspace:selection`,
		{ defaultValue: null },
	);
	const commitStackIds = getStackIdsByCommitId(headInfo);
	const branchRefsByStackId = getBranchRefsByStackId(headInfo);
	const selection =
		(_selection ? normalizeSelection(_selection, commitStackIds, branchRefsByStackId) : null) ??
		getDefaultSelection({
			headInfo,
			changes: worktreeChanges.changes,
			assignments: worktreeChanges.assignments,
		});

	const commonBaseCommitId = getCommonBaseCommitId(headInfo);

	const isUnassignedFileSelected = (path: string): boolean =>
		selection?._tag === "ChangesFile" && selection.stackId === null && selection.path === path;
	const toggleUnassignedFileSelection = (path: string) => {
		select(isUnassignedFileSelected(path) ? null : { _tag: "ChangesFile", stackId: null, path });
	};

	const isBranchSelected = (stackId: string, branchRef: string) =>
		selection?._tag === "Branch" &&
		selection.stackId === stackId &&
		selection.branchRef === branchRef;
	const toggleBranchSelection = (stackId: string, branchName: string, branchRef: string) => {
		select(
			isBranchSelected(stackId, branchRef)
				? null
				: { _tag: "Branch", stackId, branchName, branchRef },
		);
	};

	const isCommitSelected = (stackId: string, commitId: string) =>
		selection?._tag === "Commit" &&
		selection.stackId === stackId &&
		selection.commitId === commitId;
	const isCommitEditing = (stackId: string, commitId: string) =>
		selection?._tag === "Commit" &&
		selection.stackId === stackId &&
		selection.commitId === commitId &&
		selection.isEditingMessage === true;
	const isCommitSelectedWithin = (stackId: string, commitId: string) =>
		selection?._tag === "CommitFile" &&
		selection.stackId === stackId &&
		selection.commitId === commitId;
	const isChangeUnitFileSelected = (stackId: string, changeUnit: ChangeUnit, path: string) => {
		if (!selection) return false;
		if (selection._tag === "CommitFile" && changeUnit._tag === "Commit")
			return (
				selection.stackId === stackId &&
				selection.commitId === changeUnit.commitId &&
				selection.path === path
			);
		if (selection._tag === "ChangesFile" && changeUnit._tag === "Changes")
			return selection.stackId === stackId && selection.path === path;
		return false;
	};

	const toggleCommitSelection = (stackId: string, commitId: string) => {
		select(
			isCommitSelected(stackId, commitId)
				? null
				: { _tag: "Commit", stackId, commitId, isEditingMessage: false },
		);
	};
	const toggleCommitExpanded = async (stackId: string, commitId: string) => {
		if (isCommitSelectedWithin(stackId, commitId)) {
			select({ _tag: "Commit", stackId, commitId, isEditingMessage: false });
			return;
		}

		const commitDetails = await queryClient.ensureQueryData(
			commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
		);
		const firstPath = commitDetails.changes[0]?.path;

		select(
			firstPath !== undefined
				? { _tag: "CommitFile", stackId, commitId, path: firstPath }
				: { _tag: "Commit", stackId, commitId, isEditingMessage: false },
		);
	};
	const toggleChangeUnitFileSelection = (stackId: string, changeUnit: ChangeUnit, path: string) => {
		select(
			isChangeUnitFileSelected(stackId, changeUnit, path)
				? changeUnit._tag === "Commit"
					? {
							_tag: "Commit",
							stackId,
							commitId: changeUnit.commitId,
							isEditingMessage: false,
						}
					: null
				: changeUnit._tag === "Commit"
					? { _tag: "CommitFile", stackId, commitId: changeUnit.commitId, path }
					: { _tag: "ChangesFile", stackId, path },
		);
	};
	const toggleEditingMessage = (stackId: string, commitId: string) => {
		if (isCommitEditing(stackId, commitId)) {
			select((currentSelection) =>
				currentSelection?._tag === "Commit" &&
				currentSelection.stackId === stackId &&
				currentSelection.commitId === commitId &&
				currentSelection.isEditingMessage === true
					? { ...currentSelection, isEditingMessage: false }
					: currentSelection,
			);
			return;
		}

		select({ _tag: "Commit", stackId, commitId, isEditingMessage: true });
	};

	const highlightCommits = (commitIds: Array<string> | null) => {
		setHighlightedCommitIds(commitIds ? new Set(commitIds) : new Set());
	};

	useMonitorDraggedSourceItem({ projectId });

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	return (
		<ProjectPreviewLayout
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
			<div className={sharedStyles.lanes}>
				<div className={styles.unassignedChangesLane}>
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

				<div className={styles.headInfo}>
					<div className={styles.stackLanes}>
						{headInfo.stacks.map((stack) => {
							// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
							const stackId = stack.id!;

							return (
								<div key={stack.id} className={styles.stackLane}>
									<StackC
										projectId={project.id}
										stack={stack}
										isBranchSelected={isBranchSelected}
										toggleBranchSelection={toggleBranchSelection}
										isCommitSelected={(commitId) => isCommitSelected(stackId, commitId)}
										isCommitEditing={(commitId) => isCommitEditing(stackId, commitId)}
										isCommitSelectedWithin={(commitId) => isCommitSelectedWithin(stackId, commitId)}
										isChangeUnitFileSelected={(changeUnit, path) =>
											isChangeUnitFileSelected(stackId, changeUnit, path)
										}
										toggleCommitExpanded={(commitId) => toggleCommitExpanded(stackId, commitId)}
										toggleCommitSelection={(commitId) => {
											toggleCommitSelection(stackId, commitId);
										}}
										toggleEditingMessage={(commitId) => {
											toggleEditingMessage(stackId, commitId);
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

					{commonBaseCommitId !== undefined && (
						<TearOffBranchTarget className={styles.commonBaseCommit}>
							{shortCommitId(commonBaseCommitId)} (common base commit)
						</TearOffBranchTarget>
					)}
				</div>

				<TearOffBranchTarget className={styles.emptyLane} />
			</div>
		</ProjectPreviewLayout>
	);
};

export const projectIndexRoute = createRoute({
	getParentRoute: () => projectRoute,
	path: "/",
	component: ProjectPage,
});
