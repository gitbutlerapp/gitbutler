import { classes } from "#ui/classes.ts";
import { type FileParent } from "#ui/domain/FileParent.ts";
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
import { Commit, DiffHunk, TreeChange } from "@gitbutler/but-sdk";
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
	useResolveOperationSource,
} from "./-OperationSource.ts";

export const BranchSource: FC<
	{
		branchRef: Array<number> | null;
		branchName: string;
		isActive?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ branchRef, branchName, isActive: isActiveProp = false, render, ...props }) => {
	const dragData = branchRef ? getDragData({ _tag: "Branch", ref: branchRef }) : null;
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () => dragData ?? {},
		preview: <DragPreview>{branchName}</DragPreview>,
		canDrag: () => dragData !== null,
	});
	const isActive = isDragging || isActiveProp;

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
		isActive?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ commit, isEnabled = true, isActive: isActiveProp = false, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () => getDragData({ _tag: "Commit", commitId: commit.id }),
		preview: (
			<DragPreview>
				<CommitLabel commit={commit} />
			</DragPreview>
		),
		canDrag: () => isEnabled,
	});
	const isActive = isDragging || isActiveProp;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const CommitFileSource: FC<
	{
		change: TreeChange;
		fileParent: FileParent;
		isActive?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ change, fileParent, isActive: isActiveProp = false, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () => getDragData({ _tag: "File", parent: fileParent, path: change.path }),
		preview: <DragPreview>{change.path}</DragPreview>,
	});
	const isActive = isDragging || isActiveProp;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const ChangesFileSource: FC<
	{
		change: TreeChange;
		fileParent: FileParent;
		isActive?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ change, fileParent, isActive: isActiveProp = false, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () => getDragData({ _tag: "File", parent: fileParent, path: change.path }),
		preview: <DragPreview>{change.path}</DragPreview>,
	});
	const isActive = isDragging || isActiveProp;

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isActive && styles.activeSource),
		}),
	});
};

export const ChangesSectionSource: FC<
	{
		stackId: string | null;
		label: string;
		changeCount: number;
		isActive?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ stackId, label, changeCount, isActive: isActiveProp = false, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () => getDragData({ _tag: "ChangesSection", stackId }),
		preview: <DragPreview>{label}</DragPreview>,
		canDrag: () => changeCount > 0,
	});
	const isActive = isDragging || isActiveProp;

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
		fileParent: FileParent;
		change: TreeChange;
		hunk: DiffHunk;
		isActive?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ patch, fileParent, change, hunk, isActive: isActiveProp = false, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: () =>
			getDragData({
				_tag: "Hunk",
				parent: fileParent,
				path: change.path,
				hunkHeader: hunk,
			}),
		preview: <DragPreview>Hunk {formatHunkHeader(hunk)}</DragPreview>,
		canDrag: () => !patch.subject.isResultOfBinaryToTextConversion,
	});
	const isActive = isDragging || isActiveProp;

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

export const ChangesSectionTarget: FC<
	{
		projectId: string;
		stackId: string | null;
	} & useRender.ComponentProps<"div">
> = ({ projectId, stackId, render, ...props }) => {
	const resolveOperationSource = useResolveOperationSource(projectId);
	const [operation, dropRef] = useDroppable(({ source }) => {
		const operationSourceRef = parseDragData(source.data);
		if (!operationSourceRef) return null;

		const operationSource = resolveOperationSource(operationSourceRef);
		if (!operationSource) return null;

		return getCombineOperation({
			operationSource,
			target: { _tag: "ChangesSection", stackId },
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
		activeOperation?: Operation | null;
		commitId: string;
		previousCommitId: string | undefined;
		nextCommitId: string | undefined;
		projectId: string;
	} & useRender.ComponentProps<"div">
> = ({
	activeOperation: activeOperationProp = null,
	commitId,
	previousCommitId,
	nextCommitId,
	projectId,
	render,
	...props
}) => {
	const resolveOperationSource = useResolveOperationSource(projectId);
	const [dragOperation, dropRef] = useDroppable(({ source, input, element }) => {
		const operationSourceRef = parseDragData(source.data);
		if (!operationSourceRef) return null;

		const operationSource = resolveOperationSource(operationSourceRef);
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
	const operation = dragOperation ?? activeOperationProp;

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
		activeOperation?: Operation | null;
		branchRef: Array<number> | null;
		firstCommitId: string | undefined;
		projectId: string;
	} & useRender.ComponentProps<"div">
> = ({
	activeOperation: activeOperationProp = null,
	branchRef,
	firstCommitId,
	projectId,
	render,
	...props
}) => {
	const resolveOperationSource = useResolveOperationSource(projectId);
	const [dragOperation, dropRef] = useDroppable(({ source }) => {
		const operationSourceRef = parseDragData(source.data);
		if (!operationSourceRef) return null;

		const operationSource = resolveOperationSource(operationSourceRef);
		if (!operationSource) return null;

		return getBranchTargetOperation({ operationSource, branchRef, firstCommitId });
	});
	const operation = dragOperation ?? activeOperationProp;

	const target = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(operation && styles.activeTarget),
		}),
	});

	return <OperationTooltip operation={operation} render={target} />;
};

export const TearOffBranchTarget: FC<{ projectId: string } & useRender.ComponentProps<"div">> = ({
	projectId,
	render,
	...props
}) => {
	const resolveOperationSource = useResolveOperationSource(projectId);
	const [operation, dropRef] = useDroppable(({ source }): Operation | null => {
		const operationSourceRef = parseDragData(source.data);
		if (!operationSourceRef) return null;

		const operationSource = resolveOperationSource(operationSourceRef);
		if (!operationSource) return null;

		if (operationSource._tag !== "Branch") return null;

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
