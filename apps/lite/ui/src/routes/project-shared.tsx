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
import { ComponentProps, FC, ReactNode, startTransition, useOptimistic, useState } from "react";
import styles from "./project-shared.module.css";
import {
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/queries.ts";
import { type ChangeUnit } from "#ui/ChangeUnit.ts";
import { useDraggable } from "#ui/hooks/useDraggable.tsx";
import { commitInsertBlankMutationOptions, commitRewordMutationOptions } from "#ui/mutations.ts";

/** @public */
export const assert = <T,>(t: T | null | undefined): T => {
	if (t == null) throw new Error("Expected value to be non-null and defined");
	return t;
};

/**
 * @example
 * classes("foo", undefined, "bar", "", "baz") === "foo bar baz"
 */
export const classes = (...xs: Array<string | null | undefined | false>): string =>
	// oxlint-disable-next-line typescript/strict-boolean-expressions
	xs.reduce((acc: string, x) => (x ? (acc ? `${acc} ${x}` : x) : acc), "");

type Patch = Extract<UnifiedPatch, { type: "Patch" }>;

export type SourceItem =
	| { _tag: "Commit"; commitId: string }
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
}> = ({ diff }) => <pre className={styles.hunkDiff}>{diff.split("\n").slice(1).join("\n")}</pre>;

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
				<ul className={styles.hunks}>
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
		isSelected: boolean;
		toggleSelect: () => void;
	} & ComponentProps<"button">
> = ({ change, isSelected, toggleSelect, className, ...restProps }) => (
	<button
		{...restProps}
		type="button"
		className={classes(className, styles.fileButton, isSelected && styles.selected)}
		onClick={toggleSelect}
	>
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
					<ul className={styles.fileList}>
						{conflictedPaths.map((path) => (
							<li key={path}>{path}</li>
						))}
					</ul>
				</div>
			)}

			{data.changes.length > 0 && (
				<ul className={styles.fileList}>
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
	} & useRender.ComponentProps<"div">
> = ({ commit, render, ...props }) => {
	const { id: commitId } = commit;
	const [isDragging, dragRef] = useDraggable({
		getInitialData: (): DragData => ({
			sourceItem: { _tag: "Commit", commitId },
		}),
		preview: (
			<DragPreview>
				<CommitLabel commit={commit} />
			</DragPreview>
		),
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
	isSelected: boolean;
	isAnyFileSelected: boolean;
	onExit: () => void;
}> = ({
	projectId,
	commitId,
	message,
	setMessageAction,
	isSelected,
	isAnyFileSelected,
	onExit,
}) => {
	const commitReword = useMutation(commitRewordMutationOptions);
	const initialMessage = message.trim();

	return (
		<textarea
			ref={(el) => {
				if (!el) return;
				el.focus();
				const cursorPosition = el.value.length;
				el.setSelectionRange(cursorPosition, cursorPosition);
			}}
			defaultValue={initialMessage}
			className={classes(
				styles.editCommitMessageInput,
				isSelected ? styles.selected : isAnyFileSelected ? styles.selectedWithin : undefined,
			)}
			onBlur={onExit}
			onKeyDown={(event) => {
				if (event.key === "Escape") {
					event.preventDefault();
					onExit();
				} else if (event.key === "Enter" && !event.shiftKey) {
					event.preventDefault();
					onExit();

					const newMessage = event.currentTarget.value.trim();

					if (newMessage !== initialMessage)
						startTransition(async () => {
							await setMessageAction(newMessage);
							await commitReword.mutateAsync({
								projectId,
								commitId,
								message: newMessage,
							});
						});
				}
			}}
		/>
	);
};

const CommitMenuPopup: FC<{
	onReword: () => void;
	onInsertBlank: (side: "above" | "below") => void;
	parts: typeof Menu | typeof ContextMenu;
}> = ({ onReword, onInsertBlank, parts }) => {
	const { Popup, Item, SubmenuRoot, SubmenuTrigger, Positioner } = parts;

	return (
		<Popup className={styles.menuPopup}>
			<Item className={styles.menuItem} onClick={onReword}>
				Edit commit message
			</Item>
			<SubmenuRoot>
				<SubmenuTrigger className={styles.menuItem}>Add empty commit</SubmenuTrigger>
				<Positioner>
					<Popup className={styles.menuPopup}>
						<Item
							className={styles.menuItem}
							onClick={() => {
								onInsertBlank("above");
							}}
						>
							Above
						</Item>
						<Item
							className={styles.menuItem}
							onClick={() => {
								onInsertBlank("below");
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
		isAnyFileSelected: boolean;
		isHighlighted: boolean;
		toggleSelect: () => void;
	} & ComponentProps<"div">
> = ({
	projectId,
	commit,
	isSelected,
	isAnyFileSelected,
	isHighlighted,
	toggleSelect,
	className,
	...restProps
}) => {
	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const [isEditingMessage, setIsEditingMessage] = useState(false);
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};

	const insertBlankCommit = (side: "above" | "below") => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side,
		});
	};

	return (
		<div {...restProps} className={classes(styles.commitRow, className)}>
			{isEditingMessage ? (
				<InlineCommitMessageEditor
					projectId={projectId}
					commitId={commit.id}
					message={optimisticMessage}
					setMessageAction={setOptimisticMessage}
					isSelected={isSelected}
					isAnyFileSelected={isAnyFileSelected}
					onExit={() => {
						setIsEditingMessage(false);
					}}
				/>
			) : (
				<ContextMenu.Root>
					<ContextMenu.Trigger
						render={
							<DraggableCommit
								commit={commitWithOptimisticMessage}
								render={
									<button
										type="button"
										className={classes(
											styles.commitButton,
											isSelected
												? styles.selected
												: isAnyFileSelected
													? styles.selectedWithin
													: undefined,
										)}
										onClick={toggleSelect}
										style={{
											...(isHighlighted && { backgroundColor: "yellow" }),
										}}
									>
										<CommitLabel commit={commitWithOptimisticMessage} />
									</button>
								}
							/>
						}
					/>
					<ContextMenu.Portal>
						<ContextMenu.Positioner>
							<CommitMenuPopup
								onReword={() => setIsEditingMessage(true)}
								onInsertBlank={insertBlankCommit}
								parts={ContextMenu}
							/>
						</ContextMenu.Positioner>
					</ContextMenu.Portal>
				</ContextMenu.Root>
			)}
			<Menu.Root>
				<Menu.Trigger style={{ lineHeight: 1 }}>𑁔</Menu.Trigger>
				<Menu.Portal>
					<Menu.Positioner align="end">
						<CommitMenuPopup
							onReword={() => setIsEditingMessage(true)}
							onInsertBlank={insertBlankCommit}
							parts={Menu}
						/>
					</Menu.Positioner>
				</Menu.Portal>
			</Menu.Root>
		</div>
	);
};

export const CommitsList: FC<{
	commits: Array<Commit>;
	children: (commit: Commit, index: number) => ReactNode;
}> = ({ commits, children }) => {
	if (commits.length === 0) return <div>No commits.</div>;

	return (
		<ul className={styles.commitsList}>
			{commits.map((commit, index) => (
				<li key={commit.id}>{children(commit, index)}</li>
			))}
		</ul>
	);
};
