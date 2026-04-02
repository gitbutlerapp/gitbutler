import { classes } from "#ui/classes.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import { useDroppable } from "#ui/hooks/useDroppable.ts";
import { isCombineOperation, getInsertionSide, operationLabel, Operation } from "#ui/Operation.ts";
import {
	CommitLabel,
	decodeRefName,
	formatHunkHeader,
	Patch,
} from "#ui/routes/project/$id/-shared.tsx";
import uiStyles from "#ui/ui.module.css";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { Commit, DiffHunk, HunkAssignment, HunkHeader, TreeChange } from "@gitbutler/but-sdk";
import { Match, pipe } from "effect";
import { FC } from "react";
import {
	DragPreview,
	getCommitTargetInstruction,
	getDragData,
	parseDragData,
} from "./-DragAndDrop.tsx";
import styles from "./route.module.css";
import {
	CommitTargetAction,
	getBranchTargetOperation,
	getCombineOperation,
	getCommitTargetOperation,
} from "./-SourceItem.ts";

export type TreeChangeWithAssignments = {
	change: TreeChange;
	assignments?: Array<HunkAssignment>;
};

const hunkHeadersForAssignments = (
	assignments: Array<HunkAssignment> | undefined,
): Array<HunkHeader> =>
	assignments
		? assignments.flatMap((assignment) =>
				assignment.hunkHeader != null ? [assignment.hunkHeader] : [],
			)
		: [];

export const BranchSource: FC<
	{
		branchRef: Array<number> | null;
		branchName: string;
	} & useRender.ComponentProps<"div">
> = ({ branchRef, branchName, render, ...props }) => {
	const dragData = getDragData(branchRef !== null ? { _tag: "Branch", ref: branchRef } : null);
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () => dragData ?? {},
		preview: <DragPreview>{branchName}</DragPreview>,
		canDrag: () => dragData !== null,
	});
	const isActive = isDragging;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const CommitSource: FC<
	{
		commit: Commit;
		isEnabled?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ commit, isEnabled = true, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () => getDragData({ _tag: "Commit", commitId: commit.id }) ?? {},
		preview: (
			<DragPreview>
				<CommitLabel commit={commit} />
			</DragPreview>
		),
		canDrag: () => isEnabled,
	});
	const isActive = isDragging;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const FileSource: FC<
	{
		change: TreeChange;
		changeUnit: ChangeUnit;
		assignments?: Array<HunkAssignment>;
	} & useRender.ComponentProps<"div">
> = ({ change, changeUnit, assignments, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () =>
			getDragData({
				_tag: "TreeChanges",
				parent: changeUnit,
				changes: [
					{
						change,
						hunkHeaders: hunkHeadersForAssignments(assignments),
					},
				],
			}) ?? {},
		preview: <DragPreview>{change.path}</DragPreview>,
	});
	const isActive = isDragging;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const ChangesSource: FC<
	{
		changeUnit: ChangeUnit;
		label: string;
		changes: Array<TreeChangeWithAssignments>;
	} & useRender.ComponentProps<"div">
> = ({ changeUnit, label, changes, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () =>
			getDragData({
				_tag: "TreeChanges",
				parent: changeUnit,
				changes: changes.map(({ change, assignments }) => ({
					change,
					hunkHeaders: hunkHeadersForAssignments(assignments),
				})),
			}) ?? {},
		preview: <DragPreview>{label}</DragPreview>,
		canDrag: () => changes.length > 0,
	});
	const isActive = isDragging;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const HunkSource: FC<
	{
		patch: Patch;
		changeUnit: ChangeUnit;
		change: TreeChange;
		hunk: DiffHunk;
	} & useRender.ComponentProps<"div">
> = ({ patch, changeUnit, change, hunk, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () =>
			getDragData({
				_tag: "TreeChanges",
				parent: changeUnit,
				changes: [
					{
						change,
						hunkHeaders: [hunk],
					},
				],
			}) ?? {},
		preview: <DragPreview>Hunk {formatHunkHeader(hunk)}</DragPreview>,
		canDrag: () => !patch.subject.isResultOfBinaryToTextConversion,
	});
	const isActive = isDragging;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const ChangesTarget: FC<
	{
		stackId: string | null;
	} & useRender.ComponentProps<"div">
> = ({ stackId, render, ...props }) => {
	const [operation, dropRef] = useDroppable(({ source }) => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem) return null;
		return getCombineOperation({
			sourceItem,
			target: { _tag: "Changes", stackId },
		});
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && styles.activeTarget),
		}),
	});
	const tooltip = operation ? operationLabel(operation) : null;

	return (
		<Tooltip.Root
			open={tooltip !== null}
			onOpenChange={(_open, eventDetails) => {
				eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger render={droppable} />
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

export const CommitTarget: FC<
	{
		commitId: string;
		previousCommitId: string | undefined;
		nextCommitId: string | undefined;
	} & useRender.ComponentProps<"div">
> = ({ commitId, previousCommitId, nextCommitId, render, ...props }) => {
	const [operation, dropRef] = useDroppable(({ source, input, element }) => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem) return null;

		const instruction = getCommitTargetInstruction({
			sourceItem,
			commitId,
			previousCommitId,
			nextCommitId,
			input,
			element,
		});

		if (!instruction) return null;

		return getCommitTargetOperation({
			sourceItem,
			commitId,
			action: Match.value(instruction.operation).pipe(
				Match.withReturnType<CommitTargetAction>(),
				Match.when("combine", () => "combine"),
				Match.when("reorder-before", () => "insertAbove"),
				Match.when("reorder-after", () => "insertBelow"),
				Match.exhaustive,
			),
		});
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && isCombineOperation(operation) && styles.activeTarget),
		}),
	});

	const tooltip = operation ? operationLabel(operation) : null;

	const insertionSide = operation ? getInsertionSide(operation) : null;

	return (
		<div className={styles.commit}>
			<Tooltip.Root
				open={!!operation && isCombineOperation(operation) && tooltip !== null}
				onOpenChange={(_open, eventDetails) => {
					eventDetails.allowPropagation();
				}}
			>
				<Tooltip.Trigger render={droppable} />
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={8}>
						<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
							{tooltip}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>

			{insertionSide !== null && (
				<Tooltip.Root open disableHoverablePopup>
					<Tooltip.Trigger
						render={
							<div
								className={classes(
									styles.commitInsertionTarget,
									pipe(
										insertionSide,
										Match.value,
										Match.when("above", () => styles.commitInsertionTargetAbove),
										Match.when("below", () => styles.commitInsertionTargetBelow),
										Match.exhaustive,
									),
								)}
							/>
						}
					/>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={8}>
							<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
								{tooltip}
							</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
			)}
		</div>
	);
};

export const BranchTarget: FC<
	{
		branchRef: Array<number> | null;
		firstCommitId: string | undefined;
	} & useRender.ComponentProps<"div">
> = ({ branchRef, firstCommitId, render, ...props }) => {
	const [operation, dropRef] = useDroppable(({ source }) => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem) return null;
		return getBranchTargetOperation({ sourceItem, branchRef, firstCommitId });
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && styles.activeTarget),
		}),
	});
	const tooltip = operation ? operationLabel(operation) : null;

	return (
		<Tooltip.Root
			open={tooltip !== null}
			onOpenChange={(_open, eventDetails) => {
				eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger render={droppable} />
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

export const TearOffBranchTarget: FC<useRender.ComponentProps<"div">> = ({ render, ...props }) => {
	const [operation, dropRef] = useDroppable(({ source }): Operation | null => {
		const sourceItem = parseDragData(source.data);
		if (!sourceItem || sourceItem._tag !== "Branch") return null;
		return {
			_tag: "TearOffBranch",
			subjectBranch: decodeRefName(sourceItem.ref),
		};
	});

	const droppable = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && styles.activeTarget),
		}),
	});

	return (
		<Tooltip.Root
			open={operation !== null}
			onOpenChange={(_open, eventDetails) => {
				eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger render={droppable} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
						Tear off branch
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
