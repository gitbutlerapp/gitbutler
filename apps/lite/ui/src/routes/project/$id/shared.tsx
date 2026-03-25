import { ContextMenu, Menu, mergeProps, useRender } from "@base-ui/react";
import {
	Commit,
	DiffHunk,
	HunkAssignment,
	HunkHeader,
	TreeChange,
	UnifiedPatch,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import {
	ComponentProps,
	FC,
	ReactNode,
	startTransition,
	useOptimistic,
	useTransition,
} from "react";
import styles from "./shared.module.css";
import { classes } from "#ui/classes.ts";
import { ExpandCollapseIcon, MenuTriggerIcon } from "#ui/components/icons.tsx";
import {
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import {
	commitInsertBlankMutationOptions,
	commitRewordMutationOptions,
} from "#ui/api/mutations.ts";

/** @public */
export const assert = <T,>(t: T | null | undefined): T => {
	if (t == null) throw new Error("Expected value to be non-null and defined");
	return t;
};

type Patch = Extract<UnifiedPatch, { type: "Patch" }>;

export type SourceItem =
	| { _tag: "Commit"; commitId: string }
	| { _tag: "Branch"; anchorRef: Array<number> }
	| {
			_tag: "TreeChange";
			source: {
				parent: ChangeUnit;
				change: TreeChange;
				hunkHeaders: Array<HunkHeader>;
			};
	  };

export type DragData = {
	sourceItem: SourceItem;
};

const hunkHeaderEquals = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart === b.oldStart &&
	a.oldLines === b.oldLines &&
	a.newStart === b.newStart &&
	a.newLines === b.newLines;

const formatHunkHeader = (hunk: HunkHeader): string =>
	`-${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines}`;

export const DragPreview: FC<{
	children: ReactNode;
}> = ({ children }) => <div className={styles.dragPreview}>{children}</div>;

const assignedHunks = (
	hunks: Array<DiffHunk>,
	assignments: Array<HunkAssignment>,
): Array<DiffHunk> => {
	if (assignments.length === 0) return [];
	if (assignments.some((assignment) => assignment.hunkHeader == null)) return hunks;

	return hunks.filter((hunk) =>
		assignments.some(
			(assignment) =>
				assignment.hunkHeader != null && hunkHeaderEquals(hunk, assignment.hunkHeader),
		),
	);
};

const DraggableHunk: FC<
	{
		patch: Patch;
		changeUnit: ChangeUnit;
		change: TreeChange;
		hunk: DiffHunk;
	} & useRender.ComponentProps<"div">
> = ({ patch, changeUnit, change, hunk, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({
			sourceItem: {
				_tag: "TreeChange",
				source: {
					parent: changeUnit,
					change,
					hunkHeaders: [hunk],
				},
			},
		}),
		preview: <DragPreview>Hunk {formatHunkHeader(hunk)}</DragPreview>,
		canDrag: () => !patch.subject.isResultOfBinaryToTextConversion,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && styles.dragging),
		}),
	});
};

const HunkDiff: FC<{
	diff: string;
}> = ({ diff }) => <pre>{diff.split("\n").slice(1).join("\n")}</pre>;

export const Hunk: FC<{
	patch: Patch;
	changeUnit: ChangeUnit;
	change: TreeChange;
	hunk: DiffHunk;
	headerStart?: ReactNode;
}> = ({ patch, changeUnit, change, hunk, headerStart }) => (
	<div>
		<div className={styles.hunkHeaderRow}>
			{headerStart}
			<DraggableHunk
				patch={patch}
				changeUnit={changeUnit}
				change={change}
				hunk={hunk}
				className={styles.hunkHeader}
			>
				{formatHunkHeader(hunk)}
			</DraggableHunk>
		</div>
		<HunkDiff diff={hunk.diff} />
	</div>
);

const hunkKey = (hunk: HunkHeader): string =>
	`${hunk.oldStart}:${hunk.oldLines}:${hunk.newStart}:${hunk.newLines}`;

export const FileDiff: FC<{
	projectId: string;
	change: TreeChange;
	assignments?: Array<HunkAssignment>;
	renderHunk: (hunk: DiffHunk, patch: Patch) => ReactNode;
}> = ({ projectId, change, assignments, renderHunk }) => {
	const { data } = useSuspenseQuery(treeChangeDiffsQueryOptions({ projectId, change }));

	return Match.value(data).pipe(
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
					{visibleHunks.map((hunk) => (
						<li key={hunkKey(hunk)}>{renderHunk(hunk, patch)}</li>
					))}
				</ul>
			);
		}),
		Match.exhaustive,
	);
};

export const FileButton: FC<
	{
		change: TreeChange;
		toggleSelect: () => void;
	} & ComponentProps<"button">
> = ({ change, toggleSelect, className, ...restProps }) => (
	<button
		{...restProps}
		type="button"
		className={classes(className, styles.fileButton)}
		onClick={toggleSelect}
	>
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

export const CommitDetails: FC<{
	projectId: string;
	commitId: string;
	renderFile: (change: TreeChange) => ReactNode;
}> = ({ projectId, commitId, renderFile }) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	const conflictedPaths = data.conflictEntries
		? Array.from(
				new Set([
					...data.conflictEntries.ancestorEntries,
					...data.conflictEntries.ourEntries,
					...data.conflictEntries.theirEntries,
				]),
			).sort((a, b) => a.localeCompare(b))
		: [];

	if (conflictedPaths.length === 0 && data.changes.length === 0) return <div>No file changes.</div>;

	return (
		<>
			{conflictedPaths.length > 0 && (
				<div>
					<div>Conflicts:</div>
					<ul>
						{conflictedPaths.map((path) => (
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

export const CommitLabel: FC<{
	commit: Commit;
}> = ({ commit }) => (
	<>
		{commit.message === "" ? <>(no message)</> : commit.message.split("\n")[0]}
		{commit.hasConflicts && " ⚠️"}
	</>
);

const DraggableCommit: FC<
	{
		commit: Commit;
		canDrag?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ commit, canDrag = true, render, ...props }) => {
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({
			sourceItem: { _tag: "Commit", commitId: commit.id },
		}),
		preview: (
			<DragPreview>
				<CommitLabel commit={commit} />
			</DragPreview>
		),
		canDrag: () => canDrag,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && styles.dragging),
		}),
	});
};

export const DraggableBranch: FC<
	{
		anchorRef: Array<number> | null;
		label: string;
	} & useRender.ComponentProps<"div">
> = ({ anchorRef, label, render, ...props }) => {
	const dragData: DragData | null =
		anchorRef !== null ? { sourceItem: { _tag: "Branch", anchorRef } } : null;
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData | {} => dragData ?? {},
		preview: <DragPreview>{label}</DragPreview>,
		canDrag: () => dragData !== null,
	});

	return useRender({
		render,
		ref: dragRef,
		props: mergeProps<"div">(props, {
			className: classes(isDragging && styles.dragging),
		}),
	});
};

const InlineCommitMessageEditor: FC<{
	projectId: string;
	commitId: string;
	message: string;
	setMessageAction: (message: string) => void | Promise<void>;
	onExit: () => void;
}> = ({ projectId, commitId, message, setMessageAction, onExit }) => {
	const commitReword = useMutation(commitRewordMutationOptions);
	const initialMessage = message.trim();

	const saveMessage = (newMessage: string) => {
		onExit();
		const trimmed = newMessage.trim();
		if (trimmed !== initialMessage)
			startTransition(async () => {
				await setMessageAction(trimmed);
				await commitReword.mutateAsync({
					projectId,
					commitId,
					message: trimmed,
				});
			});
	};

	return (
		<form
			className={styles.editCommitMessageForm}
			onSubmit={(event) => {
				event.preventDefault();
				const formData = new FormData(event.currentTarget);
				saveMessage(formData.get("message") as string);
			}}
		>
			<textarea
				ref={(el) => {
					if (!el) return;
					el.focus();
					const cursorPosition = el.value.length;
					el.setSelectionRange(cursorPosition, cursorPosition);
				}}
				name="message"
				defaultValue={initialMessage}
				className={styles.editCommitMessageInput}
				onKeyDown={(event) => {
					if (event.key === "Escape") {
						event.preventDefault();
						onExit();
					} else if (event.key === "Enter" && !event.shiftKey) {
						event.preventDefault();
						event.currentTarget.form?.requestSubmit();
					}
				}}
			/>
			<div className={styles.editCommitMessageHelp}>
				<span>escape to </span>
				<button type="button" className={styles.editCommitMessageAction} onClick={onExit}>
					cancel
				</button>
				<span> • enter to </span>
				<button type="submit" className={styles.editCommitMessageAction}>
					save
				</button>
			</div>
		</form>
	);
};

const CommitMenuPopup: FC<{
	projectId: string;
	commitId: string;
	onReword: () => void;
	parts: typeof Menu | typeof ContextMenu;
}> = ({ projectId, commitId, onReword, parts }) => {
	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const { Popup, Item, SubmenuRoot, SubmenuTrigger, Positioner } = parts;

	return (
		<Popup className={styles.menuPopup}>
			<Item className={styles.menuItem} onClick={onReword}>
				Reword commit
			</Item>
			<SubmenuRoot>
				<SubmenuTrigger className={styles.menuItem}>Add empty commit</SubmenuTrigger>
				<Positioner>
					<Popup className={styles.menuPopup}>
						<Item
							className={styles.menuItem}
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
							className={styles.menuItem}
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
		</Popup>
	);
};

export const CommitRow: FC<
	{
		projectId: string;
		commit: Commit;
		isSelected: boolean;
		isSelectedWithin: boolean;
		isHighlighted: boolean;
		isEditingMessage: boolean;
		toggleExpand: () => Promise<void> | void;
		toggleSelect: () => void;
		toggleEditingMessage: () => void;
	} & ComponentProps<"div">
> = ({
	projectId,
	commit,
	isSelected,
	isSelectedWithin,
	isHighlighted,
	isEditingMessage,
	toggleExpand,
	toggleSelect,
	toggleEditingMessage,
	className,
	...restProps
}) => {
	const [isExpandPending, startExpandTransition] = useTransition();
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};

	return (
		<DraggableCommit
			{...restProps}
			canDrag={!isEditingMessage}
			commit={commitWithOptimisticMessage}
			render={
				<div
					className={classes(
						styles.row,
						styles.commitRow,
						isSelected ? styles.selected : isSelectedWithin ? styles.selectedWithin : undefined,
						isHighlighted && styles.highlighted,
						className,
					)}
					style={{ ...(isExpandPending && { opacity: 0.5 }) }}
					aria-busy={isExpandPending}
				>
					{isEditingMessage ? (
						<InlineCommitMessageEditor
							projectId={projectId}
							commitId={commit.id}
							message={optimisticMessage}
							setMessageAction={setOptimisticMessage}
							onExit={toggleEditingMessage}
						/>
					) : (
						<ContextMenu.Root>
							<ContextMenu.Trigger
								render={
									<button type="button" className={styles.commitButton} onClick={toggleSelect}>
										<CommitLabel commit={commitWithOptimisticMessage} />
									</button>
								}
							/>
							<ContextMenu.Portal>
								<ContextMenu.Positioner>
									<CommitMenuPopup
										projectId={projectId}
										commitId={commit.id}
										onReword={toggleEditingMessage}
										parts={ContextMenu}
									/>
								</ContextMenu.Positioner>
							</ContextMenu.Portal>
						</ContextMenu.Root>
					)}
					<button
						className={styles.rowAction}
						type="button"
						onClick={() => {
							startExpandTransition(toggleExpand);
						}}
						aria-expanded={isSelectedWithin}
						aria-label={isSelectedWithin ? "Collapse commit" : "Expand commit"}
					>
						<ExpandCollapseIcon isExpanded={isSelectedWithin} />
					</button>
					<Menu.Root>
						<Menu.Trigger className={styles.rowAction} aria-label="Commit menu">
							<MenuTriggerIcon />
						</Menu.Trigger>
						<Menu.Portal>
							<Menu.Positioner align="end">
								<CommitMenuPopup
									projectId={projectId}
									commitId={commit.id}
									onReword={toggleEditingMessage}
									parts={Menu}
								/>
							</Menu.Positioner>
						</Menu.Portal>
					</Menu.Root>
				</div>
			}
		/>
	);
};

export const CommitsList: FC<{
	commits: Array<Commit>;
	children: (commit: Commit, index: number) => ReactNode;
}> = ({ commits, children }) => {
	if (commits.length === 0) return <div>No commits.</div>;

	return (
		<ul>
			{commits.map((commit, index) => (
				<li key={commit.id}>{children(commit, index)}</li>
			))}
		</ul>
	);
};
