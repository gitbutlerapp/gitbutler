import { classes } from "#ui/classes.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import { useDroppable } from "#ui/hooks/useDroppable.ts";
import {
	getInsertionSide,
	isCombineOperation,
	operationLabel,
	type Operation,
} from "#ui/Operation.ts";
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
} from "./-OperationSource.ts";

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
		stackId: string | null;
		label: string;
		changes: Array<TreeChangeWithAssignments>;
	} & useRender.ComponentProps<"div">
> = ({ stackId, label, changes, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () =>
			getDragData({
				_tag: "TreeChanges",
				parent: { _tag: "Changes", stackId },
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

const OperationTooltip: FC<
	{
		operation: Operation | null;
	} & useRender.ComponentProps<"div">
> = ({ operation, render, ...props }) => {
	const tooltip = operation ? operationLabel(operation) : null;

	const trigger = useRender({
		render,
		props,
	});

	return (
		<Tooltip.Root
			open={tooltip !== null}
			disableHoverablePopup
			onOpenChange={(_open, eventDetails) => {
				eventDetails.allowPropagation();
			}}
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

export const ChangesTarget: FC<
	{
		stackId: string | null;
	} & useRender.ComponentProps<"div">
> = ({ stackId, render, ...props }) => {
	const [operation, dropRef] = useDroppable(({ source }) => {
		const operationSource = parseDragData(source.data);
		if (!operationSource) return null;
		return getCombineOperation({
			operationSource,
			target: { _tag: "Changes", stackId },
		});
	});

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && styles.activeTarget),
		}),
	});

	return <OperationTooltip operation={operation} render={target} />;
};

export const CommitTarget: FC<
	{
		commitId: string;
		previousCommitId: string | undefined;
		nextCommitId: string | undefined;
	} & useRender.ComponentProps<"div">
> = ({ commitId, previousCommitId, nextCommitId, render, ...props }) => {
	const [operation, dropRef] = useDroppable(({ source, input, element }) => {
		const operationSource = parseDragData(source.data);
		if (!operationSource) return null;

		const instruction = getCommitTargetInstruction({
			operationSource,
			commitId,
			previousCommitId,
			nextCommitId,
			input,
			element,
		});

		if (!instruction) return null;

		return getCommitTargetOperation({
			operationSource,
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

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && isCombineOperation(operation) && styles.activeTarget),
		}),
	});

	const insertionSide = operation ? getInsertionSide(operation) : null;

	return (
		<div className={styles.commit}>
			<OperationTooltip
				operation={operation && isCombineOperation(operation) ? operation : null}
				render={target}
			/>

			{insertionSide !== null && (
				<OperationTooltip
					operation={operation}
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
		const operationSource = parseDragData(source.data);
		if (!operationSource) return null;
		return getBranchTargetOperation({ operationSource, branchRef, firstCommitId });
	});

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && styles.activeTarget),
		}),
	});

	return <OperationTooltip operation={operation} render={target} />;
};

export const TearOffBranchTarget: FC<useRender.ComponentProps<"div">> = ({ render, ...props }) => {
	const [operation, dropRef] = useDroppable(({ source }): Operation | null => {
		const operationSource = parseDragData(source.data);
		if (!operationSource || operationSource._tag !== "Branch") return null;
		return {
			_tag: "TearOffBranch",
			subjectBranch: decodeRefName(operationSource.ref),
		};
	});

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && styles.activeTarget),
		}),
	});

	return <OperationTooltip operation={operation} render={target} />;
};
