import { createRoute } from "@tanstack/react-router";
import styles from "./project-index.module.css";
import sharedStyles from "./project-shared.module.css";
import { Menu } from "@base-ui/react";
import { ContextMenu } from "@base-ui/react/context-menu";
import { Tooltip } from "@base-ui/react/tooltip";
import {
	RefInfo,
	Commit,
	DiffHunk,
	HunkAssignment,
	InsertSide,
	TreeChange,
	UnifiedPatch,
	DiffSpec,
	RelativeTo,
	Stack,
	HunkDependencies,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import {
	createContext,
	Dispatch,
	DragEvent,
	FC,
	SetStateAction,
	startTransition,
	Suspense,
	use,
	useOptimistic,
	useState,
} from "react";
import { useLocalStorageState } from "#ui/hooks/useLocalStorageState.ts";
import {
	CommitButton,
	CommitDetails,
	CommitsList,
	FileButton,
	FileDiff,
	HunkDiff,
	hunkKey,
} from "#ui/routes/project-shared.tsx";
import {
	commitInsertBlankMutationOptions,
	commitMoveMutationOptions,
	commitMoveToBranchMutationOptions,
	commitMutationOptions,
	commitRewordMutationOptions,
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
import { rubOperationFor, type RubSource } from "#ui/rub.ts";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { createDiffSpec } from "#ui/DiffSpec.ts";

type HunkDependencyDiff = HunkDependencies["diffs"][number];

/**
 * @example
 * classes('foo', undefined, 'bar', '', 'baz') === 'foo bar baz'
 */
const classes = (...xs: Array<string | null | undefined | false>): string =>
	// oxlint-disable-next-line typescript/strict-boolean-expressions
	xs.reduce((acc: string, x) => (x ? (acc ? `${acc} ${x}` : x) : acc), "");

type Patch = Extract<UnifiedPatch, { type: "Patch" }>;

const assert = <T,>(value: T | null): T => {
	if (value === null) throw new Error("Expected value to exist.");
	return value;
};

const commonBaseCommitId = (headInfo: RefInfo): string | undefined => {
	const bases = headInfo.stacks
		.map((stack) => stack.base)
		.filter((base): base is string => base !== null);
	const first = bases[0];
	if (first === undefined) return undefined;
	return bases.every((base) => base === first) ? first : undefined;
};

const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

const commitMoveSideFor = ({
	event,
	sourceCommitId,
	targetCommitId,
	previousCommitId,
	nextCommitId,
}: {
	event: DragEvent<HTMLElement>;
	sourceCommitId: string;
	targetCommitId: string;
	previousCommitId: string | undefined;
	nextCommitId: string | undefined;
}): InsertSide | null => {
	const targetRect = event.currentTarget.getBoundingClientRect();

	const side: InsertSide =
		event.clientY < targetRect.top + targetRect.height / 2 ? "above" : "below";

	const isNoOp =
		sourceCommitId === targetCommitId ||
		(side === "above" && previousCommitId === sourceCommitId) ||
		(side === "below" && nextCommitId === sourceCommitId);

	return isNoOp ? null : side;
};

const rubSourceMimeType = "application/x-gitbutler-rub-source";
const commitMoveSourceIdMimeType = "application/x-gitbutler-commit-move-source-id";

type UseState<T> = [T, Dispatch<SetStateAction<T>>];

type RubSourceState = UseState<RubSource | null>;
const RubSourceStateContext = createContext<RubSourceState | null>(null);

type CommitMoveSourceIdState = UseState<string | null>;
const CommitMoveSourceIdStateContext = createContext<CommitMoveSourceIdState | null>(null);

const dragLeaveIsWithinTarget = (event: DragEvent<HTMLElement>): boolean =>
	event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget);

// https://linear.app/gitbutler/issue/GB-1128/references-in-mutations-should-use-bytes-instead-of-strings
const decodeRefName = (fullNameBytes: Array<number>): string =>
	new TextDecoder().decode(Uint8Array.from(fullNameBytes));

const stackRelativeTo = (stack: Stack): RelativeTo | null => {
	const segmentWithRef = stack.segments.find((segment) => segment.refName != null);
	if (segmentWithRef?.refName)
		return {
			type: "reference",
			subject: decodeRefName(segmentWithRef.refName.fullNameBytes),
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

const HunkListItem: FC<{
	patch: Patch;
	changeUnit: ChangeUnit;
	change: TreeChange;
	hunk: DiffHunk;
	children: React.ReactNode;
}> = ({ patch, changeUnit, change, hunk, children }) => {
	const [, setRubSource] = assert(use(RubSourceStateContext));
	const [isDragging, setIsDragging] = useState(false);

	return (
		<li
			className={classes(styles.hunkListItem, isDragging && styles.dragging)}
			draggable={!patch.subject.isResultOfBinaryToTextConversion}
			onDragStart={(event) => {
				setIsDragging(true);
				const rubSource: RubSource = {
					_tag: "FilePatch",
					source: {
						parent: changeUnit,
						change,
						hunkHeaders: [hunk],
					},
				};
				const payload = JSON.stringify(rubSource);
				setRubSource(rubSource);
				event.dataTransfer.setData(rubSourceMimeType, payload);
				event.dataTransfer.effectAllowed = "move";
			}}
			onDragEnd={() => {
				setIsDragging(false);
				setRubSource(null);
			}}
		>
			{children}
		</li>
	);
};

const ChangesHunkListItem: FC<{
	patch: Patch;
	changeUnit: ChangeUnit;
	change: TreeChange;
	hunk: DiffHunk;
	hunkDependencyDiffs: Array<HunkDependencyDiff> | undefined;
	onLockHover: (commitIds: Array<string> | null) => void;
}> = ({ patch, changeUnit, change, hunk, hunkDependencyDiffs, onLockHover }) => {
	const dependencyCommitIds = hunkDependencyDiffs
		? dependencyCommitIdsForHunk(hunk, hunkDependencyDiffs)
		: [];

	return (
		<HunkListItem patch={patch} changeUnit={changeUnit} change={change} hunk={hunk}>
			{dependencyCommitIds.length > 0 && (
				<span
					onMouseEnter={() => {
						onLockHover(dependencyCommitIds);
					}}
					onMouseLeave={() => {
						onLockHover(null);
					}}
				>
					🔒
				</span>
			)}
			<HunkDiff diff={hunk.diff} />
		</HunkListItem>
	);
};

const CommitHunkListItem: FC<{
	patch: Patch;
	changeUnit: ChangeUnit;
	change: TreeChange;
	hunk: DiffHunk;
}> = ({ patch, changeUnit, change, hunk }) => (
	<HunkListItem patch={patch} changeUnit={changeUnit} change={change} hunk={hunk}>
		<HunkDiff diff={hunk.diff} />
	</HunkListItem>
);

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
		for (const lock of locks) commitIds.add(lock.commitId);
	}

	return Array.from(commitIds);
};

const dependencyCommitIdsForFile = (
	hunkDependencyDiffs: Array<HunkDependencyDiff>,
): Array<string> => {
	const commitIds = new Set<string>();

	for (const [, , locks] of hunkDependencyDiffs)
		for (const lock of locks) commitIds.add(lock.commitId);

	return Array.from(commitIds);
};

const FileListItem: FC<{
	change: TreeChange;
	changeUnit: ChangeUnit;
	assignments?: Array<HunkAssignment>;
	children: React.ReactNode;
}> = ({ change, changeUnit, assignments, children }) => {
	const [, setRubSource] = assert(use(RubSourceStateContext));
	const [isDragging, setIsDragging] = useState(false);

	return (
		<li
			className={classes(isDragging && styles.dragging)}
			draggable
			onDragStart={(event) => {
				setIsDragging(true);
				const rubSource: RubSource = {
					_tag: "FilePatch",
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
				};
				const payload = JSON.stringify(rubSource);
				setRubSource(rubSource);
				event.dataTransfer.setData(rubSourceMimeType, payload);
				event.dataTransfer.effectAllowed = "move";
			}}
			onDragEnd={() => {
				setIsDragging(false);
				setRubSource(null);
			}}
		>
			{children}
		</li>
	);
};

const SelectedChangesFileDiff: FC<{
	projectId: string;
	stackId: string | null;
	path: string;
	onLockHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, stackId, path, onLockHover }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const assignments = assignmentsByPath.get(path);
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);
	const change = worktreeChanges.changes.find((candidate) => candidate.path === path);

	if (!assignments || !change) return null;

	return (
		<div className={sharedStyles.laneDiffPane}>
			<FileDiff
				projectId={projectId}
				change={change}
				assignments={assignments}
				renderHunk={(hunk, patch) => (
					<ChangesHunkListItem
						key={hunkKey(hunk)}
						patch={patch}
						changeUnit={{ _tag: "changes", stackId }}
						change={change}
						hunk={hunk}
						hunkDependencyDiffs={hunkDependencyDiffsByPath.get(path)}
						onLockHover={onLockHover}
					/>
				)}
			/>
		</div>
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
		<div className={sharedStyles.laneDiffPane}>
			<FileDiff
				projectId={projectId}
				change={change}
				renderHunk={(hunk, patch) => (
					<CommitHunkListItem
						key={hunkKey(hunk)}
						patch={patch}
						changeUnit={{ _tag: "commit", commitId }}
						change={change}
						hunk={hunk}
					/>
				)}
			/>
		</div>
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
		<div className={sharedStyles.laneDiffPane}>
			<ul className={sharedStyles.hunks}>
				{data.changes.map((change) => (
					<li key={change.path}>
						<h5>{change.path}</h5>
						<FileDiff
							projectId={projectId}
							change={change}
							renderHunk={(hunk, patch) => (
								<CommitHunkListItem
									key={hunkKey(hunk)}
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
		</div>
	);
};

const CommitTarget: FC<{
	projectId: string;
	commitId: string;
	previousCommitId: string | undefined;
	nextCommitId: string | undefined;
	children: React.ReactElement;
}> = ({ projectId, commitId, previousCommitId, nextCommitId, children }) => {
	const changeUnit: ChangeUnit = { _tag: "commit", commitId };

	const [rubSource, setRubSource] = assert(use(RubSourceStateContext));
	const [commitMoveSourceId, setCommitMoveSourceId] = assert(use(CommitMoveSourceIdStateContext));

	const [isDragOver, setIsDragOver] = useState(false);
	const [commitMoveSide, setCommitMoveSide] = useState<InsertSide | null>(null);

	const rubOperation = rubSource ? rubOperationFor(rubSource, changeUnit) : null;

	const rubMutation = useMutation(rubMutationOptions);
	const commitMove = useMutation(commitMoveMutationOptions);

	const tooltip = isDragOver && rubOperation !== null ? rubOperation : undefined;

	return (
		<Tooltip.Root open={tooltip !== undefined}>
			<Tooltip.Trigger
				render={children}
				onDragOver={(event) => {
					setIsDragOver(true);

					switch (true) {
						case event.dataTransfer.types.includes(commitMoveSourceIdMimeType): {
							if (commitMoveSourceId == null) return;

							const newCommitMoveSide = commitMoveSideFor({
								event,
								sourceCommitId: commitMoveSourceId,
								targetCommitId: changeUnit.commitId,
								previousCommitId,
								nextCommitId,
							});
							setCommitMoveSide(newCommitMoveSide);
							if (newCommitMoveSide === null) return;

							event.preventDefault();

							break;
						}
						case event.dataTransfer.types.includes(rubSourceMimeType): {
							if (rubOperation === null) return;

							event.preventDefault();

							break;
						}
					}
				}}
				onDragLeave={(event) => {
					if (dragLeaveIsWithinTarget(event)) return;

					setIsDragOver(false);
					setCommitMoveSide(null);
				}}
				onDrop={(event) => {
					setIsDragOver(false);

					switch (true) {
						case event.dataTransfer.types.includes(commitMoveSourceIdMimeType): {
							event.preventDefault();

							setCommitMoveSide(null);
							setCommitMoveSourceId(null);

							if (commitMoveSourceId == null) return;
							if (commitMoveSide === null) return;

							commitMove.mutate({
								projectId,
								subjectCommitId: commitMoveSourceId,
								anchorCommitId: changeUnit.commitId,
								side: commitMoveSide,
							});

							break;
						}
						case event.dataTransfer.types.includes(rubSourceMimeType): {
							event.preventDefault();

							setRubSource(null);

							if (!rubSource) return;
							if (rubOperation === null) return;

							rubMutation.mutate({
								projectId,
								source: rubSource,
								target: changeUnit,
							});

							break;
						}
					}
				}}
				style={{
					...(isDragOver && rubOperation !== null && { outline: "2px dashed" }),
					...(commitMoveSide === "above" && {
						boxShadow: "inset 0 2px 0 0 currentColor",
					}),
					...(commitMoveSide === "below" && {
						boxShadow: "inset 0 -2px 0 0 currentColor",
					}),
				}}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={styles.tooltip}>{tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
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

	return (
		<textarea
			ref={(el) => {
				if (!el) return;
				el.focus();
				const cursorPosition = el.value.length;
				el.setSelectionRange(cursorPosition, cursorPosition);
			}}
			defaultValue={message}
			className={classes(
				styles.commitMessageInput,
				isSelected
					? sharedStyles.selected
					: isAnyFileSelected
						? sharedStyles.selectedWithin
						: undefined,
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

					if (newMessage !== message)
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
	onInsertBlank: (side: InsertSide) => void;
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

const StackMenuPopup: FC<{
	projectId: string;
	stackId: string;
}> = ({ projectId, stackId }) => {
	const unapplyStack = useMutation(unapplyStackMutationOptions);

	return (
		<Menu.Popup className={styles.menuPopup}>
			<Menu.Item className={styles.menuItem} disabled>
				Move to leftmost
			</Menu.Item>
			<Menu.Item className={styles.menuItem} disabled>
				Move to rightmost
			</Menu.Item>
			<Menu.Separator />
			<Menu.Item
				className={styles.menuItem}
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
	const [, setCommitMoveSourceId] = assert(use(CommitMoveSourceIdStateContext));
	const expanded = isSelected || isAnyFileSelected;
	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const [isEditingMessage, setIsEditingMessage] = useState(false);
	const [isDragging, setIsDragging] = useState(false);

	const changeUnit: ChangeUnit = { _tag: "commit", commitId: commit.id };

	const insertBlankCommit = (side: InsertSide) => {
		commitInsertBlank.mutate({
			projectId,
			relativeTo: { type: "commit", subject: commit.id },
			side,
		});
	};

	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);

	return (
		<CommitTarget
			projectId={projectId}
			commitId={commit.id}
			previousCommitId={previousCommitId}
			nextCommitId={nextCommitId}
		>
			<li className={sharedStyles.commitsListItem}>
				<div className={styles.commitRow}>
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
									<CommitButton
										draggable
										onDragStart={(event) => {
											setIsDragging(true);
											setCommitMoveSourceId(commit.id);
											event.dataTransfer.setData(commitMoveSourceIdMimeType, commit.id);
											event.dataTransfer.effectAllowed = "move";
										}}
										onDragEnd={() => {
											setIsDragging(false);
											setCommitMoveSourceId(null);
										}}
										className={classes(isDragging && styles.dragging)}
										commit={{ ...commit, message: optimisticMessage }}
										isSelected={isSelected}
										isAnyFileSelected={isAnyFileSelected}
										isHighlighted={isHighlighted}
										toggleSelect={toggleSelect}
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
						<Menu.Trigger>m</Menu.Trigger>
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
				{expanded && (
					<div className={sharedStyles.commitDetails}>
						<Suspense fallback={<div>Loading changed details…</div>}>
							<CommitDetails
								projectId={projectId}
								commitId={commit.id}
								renderFile={(change) => (
									<FileListItem key={change.path} change={change} changeUnit={changeUnit}>
										<div className={sharedStyles.fileRow}>
											<FileButton
												change={change}
												isSelected={isFileSelected(change.path)}
												toggleSelect={() => toggleFileSelect(change.path)}
											/>
										</div>
									</FileListItem>
								)}
							/>
						</Suspense>
					</div>
				)}
			</li>
		</CommitTarget>
	);
};

const Changes: FC<{
	projectId: string;
	stackId: string | null;
	isFileSelected: (path: string) => boolean;
	toggleFileSelect: (path: string) => void;
	onLockHover?: (commitIds: Array<string> | null) => void;
	className?: string;
}> = ({ projectId, stackId, isFileSelected, toggleFileSelect, onLockHover, className }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));

	return (
		<ChangesTarget projectId={projectId} stackId={stackId}>
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
								<FileListItem
									key={change.path}
									change={change}
									changeUnit={{ _tag: "changes", stackId }}
									assignments={assignments}
								>
									<div className={sharedStyles.fileRow}>
										<FileButton
											change={change}
											isSelected={isFileSelected(change.path)}
											toggleSelect={() => {
												toggleFileSelect(change.path);
											}}
										/>
										{dependencyCommitIds.length > 0 && (
											<span
												onMouseEnter={() => {
													onLockHover?.(dependencyCommitIds);
												}}
												onMouseLeave={() => {
													onLockHover?.(null);
												}}
											>
												🔒
											</span>
										)}
									</div>
								</FileListItem>
							);
						})}
					</ul>
				)}
			</div>
		</ChangesTarget>
	);
};

const ChangesTarget: FC<{
	projectId: string;
	stackId: string | null;
	children: React.ReactElement;
}> = ({ projectId, stackId, children }) => {
	const changeUnit: ChangeUnit = { _tag: "changes", stackId };

	const [rubSource, setRubSource] = assert(use(RubSourceStateContext));
	const [isDragOver, setIsDragOver] = useState(false);
	const rubOperation = rubSource ? rubOperationFor(rubSource, changeUnit) : null;
	const rubMutation = useMutation(rubMutationOptions);

	return (
		<Tooltip.Root open={isDragOver && rubOperation !== null}>
			<Tooltip.Trigger
				render={children}
				onDragOver={(event) => {
					setIsDragOver(true);

					if (!event.dataTransfer.types.includes(rubSourceMimeType)) return;

					if (rubOperation === null) return;

					event.preventDefault();
				}}
				onDragLeave={(event) => {
					if (dragLeaveIsWithinTarget(event)) return;

					setIsDragOver(false);
				}}
				onDrop={(event) => {
					setIsDragOver(false);

					if (!event.dataTransfer.types.includes(rubSourceMimeType)) return;

					event.preventDefault();

					setRubSource(null);

					if (!rubSource) return;
					if (rubOperation === null) return;

					rubMutation.mutate({
						projectId,
						source: rubSource,
						target: changeUnit,
					});
				}}
				style={{
					...(isDragOver && rubOperation !== null && { outline: "2px dashed" }),
				}}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={styles.tooltip}>{rubOperation}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const CommitForm: FC<{
	projectId: string;
	stack: Stack;
}> = ({ projectId, stack }) => {
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
	const [message, setMessage] = useLocalStorageState(`commitMessage:${projectId}:${stack.id!}`, "");
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
	onLockHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, onLockHover }) => {
	const [selection, select] = useLocalStorageState<UnassignedLaneSelection | null>(
		`unassignedChangesLaneSelection:${projectId}`,
		null,
	);

	const isFileSelected = (path: string): boolean => selection?.path === path;

	const toggleFileSelection = (path: string): UnassignedLaneSelection | null =>
		isFileSelected(path) ? null : { path };

	return (
		<li className={sharedStyles.lane}>
			<div className={sharedStyles.laneMain}>
				<div>
					<h3>Unassigned changes</h3>
					<Changes
						projectId={projectId}
						stackId={null}
						isFileSelected={isFileSelected}
						toggleFileSelect={(path) => {
							select(toggleFileSelection(path));
						}}
						onLockHover={onLockHover}
						className={styles.unassignedChanges}
					/>
				</div>
			</div>

			{selection !== null && (
				<Suspense fallback={<div>Loading diff…</div>}>
					<SelectedChangesFileDiff
						projectId={projectId}
						stackId={null}
						path={selection.path}
						onLockHover={onLockHover}
					/>
				</Suspense>
			)}
		</li>
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

const BranchTarget: FC<{
	projectId: string;
	anchorRef: string | null;
	firstCommitId: string | undefined;
	children: React.ReactElement;
}> = ({ projectId, anchorRef, firstCommitId, children }) => {
	const [commitMoveSourceId, setCommitMoveSourceId] = assert(use(CommitMoveSourceIdStateContext));
	const [isDragOver, setIsDragOver] = useState(false);
	const isValidCommitMoveTarget =
		isDragOver &&
		commitMoveSourceId != null &&
		anchorRef !== null &&
		firstCommitId !== commitMoveSourceId;

	const commitMoveToBranch = useMutation(commitMoveToBranchMutationOptions);

	return (
		<Tooltip.Root open={isDragOver && isValidCommitMoveTarget}>
			<Tooltip.Trigger
				render={children}
				onDragOver={(event) => {
					setIsDragOver(true);

					if (!event.dataTransfer.types.includes(commitMoveSourceIdMimeType)) return;

					if (!isValidCommitMoveTarget) return;

					event.preventDefault();
				}}
				onDragLeave={(event) => {
					if (dragLeaveIsWithinTarget(event)) return;

					setIsDragOver(false);
				}}
				onDrop={(event) => {
					setIsDragOver(false);

					if (!event.dataTransfer.types.includes(commitMoveSourceIdMimeType)) return;

					event.preventDefault();

					setCommitMoveSourceId(null);
					if (commitMoveSourceId == null) return;

					if (!isValidCommitMoveTarget) return;

					commitMoveToBranch.mutate({
						projectId,
						subjectCommitId: commitMoveSourceId,
						anchorRef,
					});
				}}
				style={{
					...(isDragOver && isValidCommitMoveTarget && { outline: "2px dashed" }),
				}}
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
	onLockHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, stack, highlightedCommitIds, onLockHover }) => {
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
		`stackLaneSelection:${projectId}:${stackId}`,
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
		<li className={sharedStyles.lane}>
			<div className={sharedStyles.laneMain}>
				<Menu.Root>
					<Menu.Trigger className={styles.stackMenu}>m</Menu.Trigger>
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
						onLockHover={onLockHover}
						className={styles.assignedChanges}
					/>
					<CommitForm projectId={projectId} stack={stack} />
				</div>

				<ul className={styles.segments}>
					{stack.segments.map((segment) => {
						const branchName = segment.refName?.displayName ?? "Untitled";
						const anchorRef = segment.refName ? decodeRefName(segment.refName.fullNameBytes) : null;
						return (
							<li key={branchName}>
								<BranchTarget
									projectId={projectId}
									anchorRef={anchorRef}
									firstCommitId={segment.commits[0]?.id}
								>
									<h3>{branchName}</h3>
								</BranchTarget>

								<h4>Commits</h4>
								<CommitsList commits={segment.commits}>
									{(commit, index) => {
										const changeUnit: ChangeUnit = {
											_tag: "commit",
											commitId: commit.id,
										};
										return (
											<CommitC
												key={commit.id}
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
				<Suspense fallback={<div>Loading diff…</div>}>
					{Match.value(selection).pipe(
						Match.tag("changes", ({ path }) => (
							<SelectedChangesFileDiff
								projectId={projectId}
								stackId={stackId}
								path={path}
								onLockHover={onLockHover}
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
			)}
		</li>
	);
};

const ProjectPage: FC = () => {
	const { id } = projectRootRoute.useParams();
	const rubSourceState = useState<RubSource | null>(null);
	const commitMoveSourceIdState = useState<string | null>(null);

	const [highlightedCommitIds, setHighlightedCommitIds] = useState<Set<string>>(() => new Set());

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());

	const project = projects.find((project) => project.id === id);

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(id));

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	const baseId = commonBaseCommitId(headInfo);

	const highlightCommits = (commitIds: Array<string> | null) => {
		setHighlightedCommitIds(commitIds ? new Set(commitIds) : new Set());
	};

	return (
		<RubSourceStateContext value={rubSourceState}>
			<CommitMoveSourceIdStateContext value={commitMoveSourceIdState}>
				<h2>{project.title} workspace</h2>

				<ul className={styles.lanes}>
					<UnassignedLane projectId={project.id} onLockHover={highlightCommits} />

					{headInfo.stacks.map((stack) => (
						<StackLane
							key={stack.id}
							projectId={project.id}
							stack={stack}
							highlightedCommitIds={highlightedCommitIds}
							onLockHover={highlightCommits}
						/>
					))}
				</ul>

				{baseId !== undefined && <>{shortCommitId(baseId)} (common base commit)</>}
			</CommitMoveSourceIdStateContext>
		</RubSourceStateContext>
	);
};

export const projectIndexRoute = createRoute({
	getParentRoute: () => projectRootRoute,
	path: "/",
	component: ProjectPage,
});
