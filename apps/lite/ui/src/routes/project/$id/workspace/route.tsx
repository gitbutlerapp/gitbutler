import {
	commitCreateMutationOptions,
	commitInsertBlankMutationOptions,
	commitRewordMutationOptions,
	unapplyStackMutationOptions,
} from "#ui/api/mutations.ts";
import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import {
	DependencyIcon,
	ExpandCollapseIcon,
	MenuTriggerIcon,
	PushIcon,
} from "#ui/components/icons.tsx";
import { rejectedChangesToastOptions } from "#ui/components/RejectedChanges.tsx";
import { type ChangeUnit } from "#ui/domain/ChangeUnit.ts";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";
import {
	getBranchNameByCommitId,
	getCommonBaseCommitId,
	getSegmentBranchRef,
} from "#ui/domain/RefInfo.ts";
import { stackRelativeTo } from "#ui/domain/Stack.ts";
import { useCloseWatcher } from "#ui/hooks/useCloseWatcher.ts";
import { useDroppable } from "#ui/hooks/useDroppable.ts";
import { type Operation } from "#ui/Operation.ts";
import { ProjectPreviewLayout } from "#ui/routes/project/$id/-ProjectPreviewLayout.tsx";
import {
	DraggableBranch,
	DraggableCommit,
	DraggableFile,
	DraggableHunk,
	parseDragData,
	useMonitorDraggedSourceItem,
} from "#ui/routes/project/$id/workspace/-DragAndDrop.tsx";
import { rubOperationLabel } from "#ui/routes/project/$id/workspace/-RubOperationLabel.ts";
import { getRubOperation, type SourceItem } from "#ui/routes/project/$id/workspace/-SourceItem.ts";
import {
	CommitDetails as SharedCommitDetails,
	CommitLabel,
	CommitsList,
	FileButton,
	FileDiff,
	formatHunkHeader,
	HunkDiff,
	isTypingTarget,
	Patch,
	ShowBranch,
	ShowCommit,
} from "#ui/routes/project/$id/-shared.tsx";
import uiStyles from "#ui/ui.module.css";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { ContextMenu, Menu, mergeProps, Popover, Toast, Tooltip, useRender } from "@base-ui/react";
import {
	Commit,
	DiffHunk,
	DiffSpec,
	HunkAssignment,
	HunkDependencies,
	HunkHeader,
	InsertSide,
	type RefInfo,
	Segment,
	Stack,
	TreeChange,
} from "@gitbutler/but-sdk";
import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Array, Match, pipe } from "effect";
import { isNonEmptyArray, NonEmptyArray } from "effect/Array";
import {
	ComponentProps,
	FC,
	Fragment,
	ReactNode,
	startTransition,
	Suspense,
	useEffect,
	useEffectEvent,
	useOptimistic,
	useState,
} from "react";
import useLocalStorageState from "use-local-storage-state";
import sharedStyles from "../-shared.module.css";
import {
	getAdjacentCommitDetailsPath,
	getCommitDetailsForCommit,
	normalizeCommitDetailsSelection,
	commitDetailsSelectionBindings,
	type CommitDetailsSelection,
} from "./-CommitDetailsSelection.ts";
import {
	changesDetailsItem,
	changesSummaryItem,
	commitEditingMessageItem,
	commitSummaryItem,
	normalizeItem,
	type Item,
	segmentItem,
	getParentSection,
} from "./-Item.ts";
import {
	buildNavigationModel,
	getAdjacentItem,
	getAdjacentSection,
	changesSelectionBindings,
	commitEditingMessageBindings,
	commitSelectionBindings,
	segmentSelectionBindings,
	SharedSelectionAction,
} from "./-Selection.ts";
import { PositionedShortcutBar, getShortcutBarMode } from "./-ShortcutBar.tsx";
import { formatShortcutKeys, getShortcutAction } from "#ui/shortcuts.ts";
import styles from "./route.module.css";

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

const useSelectionKeyboardShortcuts = ({
	commitDetailsSelection,
	selection,
	select,
	selectCommitDetails,
	headInfo,
	worktreeChanges,
}: {
	selection: Item | null;
	select: (selection: Item | null) => void;
	commitDetailsSelection: CommitDetailsSelection | null;
	selectCommitDetails: (selection: CommitDetailsSelection | null) => void;
	headInfo: RefInfo;
	worktreeChanges: {
		changes: Array<TreeChange>;
		assignments: Array<HunkAssignment>;
	};
}) => {
	const navigationModel = buildNavigationModel({
		headInfo,
		changes: worktreeChanges.changes,
		assignments: worktreeChanges.assignments,
	});

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented) return;
		if (event.metaKey || event.ctrlKey || event.altKey) return;
		if (isTypingTarget(event.target)) return;

		if (selection?._tag === "Commit" && commitDetailsSelection !== null) return;

		if (!selection) return;

		const handleSharedAction = (action: SharedSelectionAction) =>
			Match.value(action).pipe(
				Match.tag("Move", ({ offset }) =>
					select(getAdjacentItem(navigationModel, selection, offset)),
				),
				Match.tag("NextSection", () => select(getAdjacentSection(navigationModel, selection, 1))),
				Match.tag("PreviousSection", () =>
					select(getParentSection(selection) ?? getAdjacentSection(navigationModel, selection, -1)),
				),
				Match.exhaustive,
			);

		Match.value(selection).pipe(
			Match.tag("Changes", (selection) => {
				const action = getShortcutAction(changesSelectionBindings, selection, event);
				if (!action) return;

				event.preventDefault();

				Match.value(action).pipe(
					Match.tagsExhaustive({
						Move: ({ offset }) => handleSharedAction({ _tag: "Move", offset }),
						NextSection: () => handleSharedAction({ _tag: "NextSection" }),
						PreviousSection: () => handleSharedAction({ _tag: "PreviousSection" }),
					}),
				);
			}),
			Match.tag("Segment", () => {
				const action = getShortcutAction(segmentSelectionBindings, undefined, event);
				if (!action) return;

				event.preventDefault();

				handleSharedAction(action);
			}),
			Match.tag("Commit", (selection) => {
				const action = getShortcutAction(commitSelectionBindings, selection, event);
				if (!action) return;

				event.preventDefault();

				Match.value(action).pipe(
					Match.tagsExhaustive({
						Move: ({ offset }) => handleSharedAction({ _tag: "Move", offset }),
						NextSection: () => handleSharedAction({ _tag: "NextSection" }),
						PreviousSection: () => handleSharedAction({ _tag: "PreviousSection" }),
						EditCommitMessage: () => select(commitEditingMessageItem(selection)),
						ExpandCommit: () =>
							selectCommitDetails({
								stackId: selection.stackId,
								segmentIndex: selection.segmentIndex,
								branchName: selection.branchName,
								branchRef: selection.branchRef,
								commitId: selection.commitId,
							}),
					}),
				);
			}),
			Match.exhaustive,
		);
	});

	useEffect(() => {
		window.addEventListener("keydown", handleKeyDown);

		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, []);
};

const CommitDetails: FC<{
	branchName: string | null;
	branchRef: string | null;
	commitId: string;
	commitDetailsSelection: CommitDetailsSelection;
	projectId: string;
	segmentIndex: number;
	selectCommitDetails: (selection: CommitDetailsSelection | null) => void;
	stackId: string;
}> = ({
	branchName,
	branchRef,
	commitId,
	commitDetailsSelection,
	projectId,
	segmentIndex,
	selectCommitDetails,
	stackId,
}) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({
			projectId,
			commitId,
		}),
	);
	const paths = commitDetails.changes.map((change: TreeChange) => change.path);
	const selectedPath =
		commitDetailsSelection.path !== undefined && paths.includes(commitDetailsSelection.path)
			? commitDetailsSelection.path
			: paths[0];

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented) return;
		if (event.metaKey || event.ctrlKey || event.altKey) return;
		if (isTypingTarget(event.target)) return;

		const action = getShortcutAction(commitDetailsSelectionBindings, undefined, event);
		if (!action) return;

		Match.value(action).pipe(
			Match.tag("Move", ({ offset }) => {
				const nextPath = getAdjacentCommitDetailsPath({
					paths,
					currentPath: selectedPath,
					offset,
				});
				event.preventDefault();
				if (nextPath !== null)
					selectCommitDetails({
						stackId,
						segmentIndex,
						branchName,
						branchRef,
						commitId,
						path: nextPath,
					});

				return;
			}),
			Match.tag("Close", () => {
				event.preventDefault();
				selectCommitDetails(null);
				return;
			}),
			Match.exhaustive,
		);
	});

	useEffect(() => {
		window.addEventListener("keydown", handleKeyDown);
		return () => {
			window.removeEventListener("keydown", handleKeyDown);
		};
	}, []);

	return (
		<SharedCommitDetails
			projectId={projectId}
			commitId={commitId}
			renderFile={(change) => (
				<DraggableFile
					change={change}
					changeUnit={{ _tag: "Commit", commitId }}
					render={
						<div
							className={classes(
								sharedStyles.item,
								selectedPath === change.path && sharedStyles.selectedFile,
							)}
						>
							<FileButton
								change={change}
								toggleSelect={() => {
									selectCommitDetails({
										stackId,
										segmentIndex,
										branchName,
										branchRef,
										commitId,
										path: change.path,
									});
								}}
							/>
						</div>
					}
				/>
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

const ShowChangesFile: FC<{
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

const ShowChanges: FC<{
	projectId: string;
	stackId: string | null;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({ projectId, stackId, onDependencyHover }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));

	if (changes.length === 0) return <div>No file changes.</div>;

	return (
		<ul>
			{changes.map((change) => (
				<li key={change.path}>
					<h4>{change.path}</h4>
					<ShowChangesFile
						projectId={projectId}
						stackId={stackId}
						path={change.path}
						onDependencyHover={onDependencyHover}
					/>
				</li>
			))}
		</ul>
	);
};

const ShowCommitFile: FC<{
	projectId: string;
	commitId: string;
	path: string;
}> = ({ projectId, commitId, path }) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	const change = commitDetails.changes.find((candidate) => candidate.path === path);

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

const Preview: FC<{
	commitDetailsSelection: CommitDetailsSelection | null;
	projectId: string;
	selection: Item;
	onDependencyHover: (commitIds: Array<string> | null) => void;
}> = ({ commitDetailsSelection, projectId, selection, onDependencyHover }) =>
	Match.value(selection).pipe(
		Match.tag("Segment", ({ branchName, branchRef }) =>
			branchName != null && branchRef != null ? (
				<ShowBranch
					projectId={projectId}
					branchRef={branchRef}
					branchName={branchName}
					remote={null}
					renderHunk={(change, hunk, patch) => (
						<Hunk
							patch={patch}
							changeUnit={{ _tag: "Changes", stackId: null }}
							change={change}
							hunk={hunk}
						/>
					)}
				/>
			) : (
				<div>
					TODO: the API doesn't provide a way to show details/diffs for segments that don't have
					branch names.
				</div>
			),
		),
		Match.tag("Changes", ({ stackId, mode }) =>
			mode._tag === "Details" && mode.path !== undefined ? (
				<ShowChangesFile
					projectId={projectId}
					stackId={stackId}
					path={mode.path}
					onDependencyHover={onDependencyHover}
				/>
			) : (
				<ShowChanges
					projectId={projectId}
					stackId={stackId}
					onDependencyHover={onDependencyHover}
				/>
			),
		),
		Match.tag("Commit", ({ commitId, stackId, segmentIndex }) =>
			commitDetailsSelection !== null &&
			commitDetailsSelection.stackId === stackId &&
			commitDetailsSelection.segmentIndex === segmentIndex &&
			commitDetailsSelection.commitId === commitId &&
			commitDetailsSelection.path !== undefined ? (
				<ShowCommitFile
					projectId={projectId}
					commitId={commitId}
					path={commitDetailsSelection.path}
				/>
			) : (
				<ShowCommit
					projectId={projectId}
					commitId={commitId}
					renderHunk={(change, hunk, patch) => (
						<Hunk
							patch={patch}
							changeUnit={{ _tag: "Commit", commitId }}
							change={change}
							hunk={hunk}
						/>
					)}
				/>
			),
		),
		Match.exhaustive,
	);

const ChangesTarget: FC<
	{
		stackId: string | null;
	} & useRender.ComponentProps<"div">
> = ({ stackId, render, ...props }) => {
	const getOperation = (sourceItem: SourceItem): Operation | null => {
		const rubOperation = getRubOperation({
			sourceItem,
			target: { _tag: "Changes", stackId },
		});
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

		const rubOperation = getRubOperation({
			sourceItem,
			target: { _tag: "Commit", commitId },
		});

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

const InlineCommitMessageEditor: FC<{
	projectId: string;
	commitId: string;
	message: string;
	setMessageAction: (message: string) => void | Promise<void>;
	onExit: () => void;
}> = ({ projectId, commitId, message, setMessageAction, onExit }) => {
	const commitReword = useMutation(commitRewordMutationOptions);
	const initialMessage = message.trim();
	const requestClose = useCloseWatcher(onExit);

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
					const action = getShortcutAction(
						commitEditingMessageBindings,
						undefined,
						event.nativeEvent,
					);
					if (!action) return;

					Match.value(action).pipe(
						Match.tag("Save", () => {
							if (event.shiftKey) return;
							event.preventDefault();
							event.currentTarget.form?.requestSubmit();
						}),
						Match.tag("Cancel", () => {
							// CloseWatcher handles this
						}),
						Match.exhaustive,
					);
				}}
			/>
			<div className={styles.editCommitMessageHelp}>
				{commitEditingMessageBindings.map((binding, index) => (
					<Fragment key={binding.id}>
						{index > 0 && " • "}
						<span>
							{formatShortcutKeys(binding.keys)} to{" "}
							{Match.value(binding.action).pipe(
								Match.tag("Cancel", () => (
									<button
										type="button"
										className={styles.editCommitMessageAction}
										onClick={() => {
											requestClose();
										}}
									>
										{binding.description}
									</button>
								)),
								Match.tag("Save", () => (
									<button type="submit" className={styles.editCommitMessageAction}>
										{binding.description}
									</button>
								)),
								Match.exhaustive,
							)}
						</span>
					</Fragment>
				))}
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
		<Popup className={sharedStyles.menuPopup}>
			<Item className={sharedStyles.menuItem} onClick={onReword}>
				Reword commit
			</Item>
			<SubmenuRoot>
				<SubmenuTrigger className={sharedStyles.menuItem}>Add empty commit</SubmenuTrigger>
				<Positioner>
					<Popup className={sharedStyles.menuPopup}>
						<Item
							className={sharedStyles.menuItem}
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
							className={sharedStyles.menuItem}
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

const CommitRow: FC<
	{
		branchName: string | null;
		branchRef: string | null;
		commit: Commit;
		isHighlighted: boolean;
		projectId: string;
		segmentIndex: number;
		selection: Item | null;
		select: (selection: Item | null) => void;
		commitDetailsSelection: CommitDetailsSelection | null;
		selectCommitDetails: (selection: CommitDetailsSelection | null) => void;
		stackId: string;
	} & ComponentProps<"div">
> = ({
	branchName,
	branchRef,
	commit,
	commitDetailsSelection,
	isHighlighted,
	projectId,
	segmentIndex,
	selection,
	select,
	selectCommitDetails,
	stackId,
	...restProps
}) => {
	const summaryItem = commitSummaryItem({
		stackId,
		segmentIndex,
		branchName,
		branchRef,
		commitId: commit.id,
	});
	const commitSelection =
		selection?._tag === "Commit" &&
		selection.stackId === stackId &&
		selection.commitId === commit.id
			? selection
			: null;
	const detailsSelection = getCommitDetailsForCommit({
		details: commitDetailsSelection,
		stackId,
		segmentIndex,
		commitId: commit.id,
	});
	const [optimisticMessage, setOptimisticMessage] = useOptimistic(
		commit.message,
		(_currentMessage, nextMessage: string) => nextMessage,
	);

	const commitWithOptimisticMessage: Commit = {
		...commit,
		message: optimisticMessage,
	};

	const toggleDetails = () => {
		select(summaryItem);

		if (detailsSelection) {
			selectCommitDetails(null);
			return;
		}

		selectCommitDetails({
			stackId,
			segmentIndex,
			branchName,
			branchRef,
			commitId: commit.id,
		});
	};

	return (
		<DraggableCommit
			{...restProps}
			canDrag={commitSelection?.mode._tag !== "EditingMessage"}
			commit={commitWithOptimisticMessage}
			render={
				<div
					className={classes(
						sharedStyles.item,
						commitSelection ? sharedStyles.selected : undefined,
						isHighlighted && sharedStyles.highlighted,
					)}
				>
					{commitSelection?.mode._tag === "EditingMessage" ? (
						<InlineCommitMessageEditor
							projectId={projectId}
							commitId={commit.id}
							message={optimisticMessage}
							setMessageAction={setOptimisticMessage}
							onExit={() => {
								select(summaryItem);
							}}
						/>
					) : (
						<ContextMenu.Root>
							<ContextMenu.Trigger
								render={
									<button
										type="button"
										className={sharedStyles.commitButton}
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
										onReword={() => {
											selectCommitDetails(null);
											select(
												commitEditingMessageItem({
													stackId,
													segmentIndex,
													branchName,
													branchRef,
													commitId: commit.id,
												}),
											);
										}}
										parts={ContextMenu}
									/>
								</ContextMenu.Positioner>
							</ContextMenu.Portal>
						</ContextMenu.Root>
					)}
					<button
						className={sharedStyles.itemAction}
						type="button"
						onClick={toggleDetails}
						aria-expanded={detailsSelection !== null}
						aria-label={detailsSelection !== null ? "Hide commit details" : "Show commit details"}
					>
						<ExpandCollapseIcon isExpanded={detailsSelection !== null} />
					</button>
					<Menu.Root>
						<Menu.Trigger className={sharedStyles.itemAction} aria-label="Commit menu">
							<MenuTriggerIcon />
						</Menu.Trigger>
						<Menu.Portal>
							<Menu.Positioner align="end">
								<CommitMenuPopup
									projectId={projectId}
									commitId={commit.id}
									onReword={() => {
										selectCommitDetails(null);
										select(
											commitEditingMessageItem({
												stackId,
												segmentIndex,
												branchName,
												branchRef,
												commitId: commit.id,
											}),
										);
									}}
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

const CommitC: FC<{
	branchName: string | null;
	branchRef: string | null;
	commit: Commit;
	isHighlighted: boolean;
	nextCommitId: string | undefined;
	previousCommitId: string | undefined;
	projectId: string;
	segmentIndex: number;
	selection: Item | null;
	select: (selection: Item | null) => void;
	commitDetailsSelection: CommitDetailsSelection | null;
	selectCommitDetails: (selection: CommitDetailsSelection | null) => void;
	stackId: string;
}> = ({
	branchName,
	branchRef,
	commit,
	commitDetailsSelection,
	isHighlighted,
	nextCommitId,
	previousCommitId,
	projectId,
	segmentIndex,
	selection,
	select,
	selectCommitDetails,
	stackId,
}) => {
	const detailsSelection = getCommitDetailsForCommit({
		details: commitDetailsSelection,
		stackId,
		segmentIndex,
		commitId: commit.id,
	});

	return (
		<CommitTarget
			commitId={commit.id}
			previousCommitId={previousCommitId}
			nextCommitId={nextCommitId}
		>
			<CommitRow
				branchName={branchName}
				branchRef={branchRef}
				commit={commit}
				commitDetailsSelection={commitDetailsSelection}
				isHighlighted={isHighlighted}
				projectId={projectId}
				segmentIndex={segmentIndex}
				selection={selection}
				select={select}
				selectCommitDetails={selectCommitDetails}
				stackId={stackId}
			/>
			{detailsSelection && (
				<div className={sharedStyles.commitDetails}>
					<Suspense fallback={<div>Loading changed details…</div>}>
						<CommitDetails
							branchName={branchName}
							branchRef={branchRef}
							commitDetailsSelection={detailsSelection}
							projectId={projectId}
							commitId={commit.id}
							segmentIndex={segmentIndex}
							selectCommitDetails={selectCommitDetails}
							stackId={stackId}
						/>
					</Suspense>
				</div>
			)}
		</CommitTarget>
	);
};

const Changes: FC<{
	label: string;
	projectId: string;
	stackId: string | null;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	selection: Item | null;
	select: (selection: Item | null) => void;
	className?: string;
}> = ({ label, projectId, stackId, onDependencyHover, selection, select, className }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const assignmentsByPath = getAssignmentsByPath(worktreeChanges.assignments, stackId);
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	const changes = worktreeChanges.changes.filter((change) => assignmentsByPath.has(change.path));
	const changesSelection =
		selection?._tag === "Changes" && selection.stackId === stackId ? selection : null;

	return (
		<ChangesTarget
			stackId={stackId}
			className={classes(className, changesSelection && sharedStyles.sectionSelected)}
		>
			<div
				className={classes(sharedStyles.item, changesSelection ? sharedStyles.selected : undefined)}
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
			</div>
			<div className={sharedStyles.commitDetails}>
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
													sharedStyles.item,
													changesSelection?.mode._tag === "Details" &&
														changesSelection.mode.path === change.path &&
														sharedStyles.selectedFile,
												)}
											>
												<FileButton
													change={change}
													toggleSelect={() => {
														select(changesDetailsItem(stackId, change.path));
													}}
												/>
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
											</div>
										}
									/>
								</li>
							);
						})}
					</ul>
				)}
			</div>
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

const SegmentRow: FC<{
	segment: Segment;
	stackId: string;
	segmentIndex: number;
	selection: Item | null;
	select: (selection: Item | null) => void;
}> = ({ segment, stackId, segmentIndex, selection, select }) => {
	const isSelected =
		selection?._tag === "Segment" &&
		selection.stackId === stackId &&
		selection.segmentIndex === segmentIndex;

	return (
		<div className={classes(sharedStyles.item, isSelected && sharedStyles.selected)}>
			<button
				type="button"
				className={styles.segmentButton}
				onClick={() => {
					select(
						segmentItem({
							stackId,
							segmentIndex,
							branchName: segment.refName?.displayName ?? null,
							branchRef: segment.refName ? getSegmentBranchRef(segment.refName) : null,
						}),
					);
				}}
			>
				{segment.refName?.displayName ?? "Untitled"}
			</button>
			<button type="button" className={sharedStyles.itemAction} aria-label="Push branch" disabled>
				<PushIcon />
			</button>
		</div>
	);
};

const SegmentC: FC<{
	highlightedCommitIds: Set<string>;
	projectId: string;
	segment: Segment;
	segmentIndex: number;
	selection: Item | null;
	select: (selection: Item | null) => void;
	commitDetailsSelection: CommitDetailsSelection | null;
	selectCommitDetails: (selection: CommitDetailsSelection | null) => void;
	stackId: string;
}> = ({
	commitDetailsSelection,
	highlightedCommitIds,
	projectId,
	segment,
	segmentIndex,
	selection,
	select,
	selectCommitDetails,
	stackId,
}) => {
	const isSelected =
		(selection?._tag === "Segment" &&
			selection.stackId === stackId &&
			selection.segmentIndex === segmentIndex) ||
		(selection?._tag === "Commit" &&
			selection.stackId === stackId &&
			segment.commits.some((commit) => commit.id === selection.commitId));

	const refName = segment.refName;

	return (
		<div className={classes(isSelected && sharedStyles.sectionSelected)}>
			{refName != null ? (
				<BranchTarget
					anchorRef={refName.fullNameBytes}
					firstCommitId={segment.commits[0]?.id}
					render={
						<DraggableBranch
							anchorRef={refName.fullNameBytes}
							branchName={refName.displayName}
							render={
								<SegmentRow
									segment={segment}
									stackId={stackId}
									segmentIndex={segmentIndex}
									selection={selection}
									select={select}
								/>
							}
						/>
					}
				/>
			) : (
				<SegmentRow
					segment={segment}
					stackId={stackId}
					segmentIndex={segmentIndex}
					selection={selection}
					select={select}
				/>
			)}

			<CommitsList commits={segment.commits}>
				{(commit, index) => (
					<CommitC
						branchName={segment.refName?.displayName ?? null}
						branchRef={segment.refName ? getSegmentBranchRef(segment.refName) : null}
						commit={commit}
						commitDetailsSelection={commitDetailsSelection}
						isHighlighted={highlightedCommitIds.has(commit.id)}
						nextCommitId={segment.commits[index + 1]?.id}
						previousCommitId={segment.commits[index - 1]?.id}
						projectId={projectId}
						segmentIndex={segmentIndex}
						selection={selection}
						select={select}
						selectCommitDetails={selectCommitDetails}
						stackId={stackId}
					/>
				)}
			</CommitsList>
		</div>
	);
};

const StackC: FC<{
	highlightedCommitIds: Set<string>;
	onDependencyHover: (commitIds: Array<string> | null) => void;
	projectId: string;
	selection: Item | null;
	select: (selection: Item | null) => void;
	commitDetailsSelection: CommitDetailsSelection | null;
	selectCommitDetails: (selection: CommitDetailsSelection | null) => void;
	stack: Stack;
}> = ({
	commitDetailsSelection,
	highlightedCommitIds,
	onDependencyHover,
	projectId,
	selection,
	select,
	selectCommitDetails,
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
							commitDetailsSelection={commitDetailsSelection}
							highlightedCommitIds={highlightedCommitIds}
							projectId={projectId}
							segment={segment}
							segmentIndex={segmentIndex}
							selection={selection}
							select={select}
							selectCommitDetails={selectCommitDetails}
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

	const [highlightedCommitIds, setHighlightedCommitIds] = useState<Set<string>>(() => new Set());

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());

	const project = projects.find((project) => project.id === projectId);

	// TODO: handle project not found error. or only run when project is not null? waterfall.
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const [_selection, select] = useLocalStorageState<Item | null>(
		`project:${projectId}:workspace:selection`,
		{ defaultValue: null },
	);
	const [_commitDetailsSelection, selectCommitDetails] =
		useLocalStorageState<CommitDetailsSelection | null>(
			`project:${projectId}:workspace:commitDetailsSelection`,
			{ defaultValue: null },
		);
	const selection =
		(_selection ? normalizeItem(_selection, headInfo) : null) ??
		buildNavigationModel({
			headInfo,
			changes: worktreeChanges.changes,
			assignments: worktreeChanges.assignments,
		}).items[0] ??
		null;
	const commitDetailsSelection = normalizeCommitDetailsSelection(_commitDetailsSelection, headInfo);

	const commonBaseCommitId = getCommonBaseCommitId(headInfo);
	const highlightCommits = (commitIds: Array<string> | null) => {
		setHighlightedCommitIds(commitIds ? new Set(commitIds) : new Set());
	};

	useMonitorDraggedSourceItem({ projectId });
	useSelectionKeyboardShortcuts({
		commitDetailsSelection,
		selection,
		select,
		selectCommitDetails,
		headInfo,
		worktreeChanges,
	});

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	return (
		<ProjectPreviewLayout
			projectId={projectId}
			preview={
				selection && (
					<Suspense fallback={<div>Loading diff…</div>}>
						<Preview
							commitDetailsSelection={commitDetailsSelection}
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
					<Changes
						label="Unassigned changes"
						projectId={project.id}
						stackId={null}
						onDependencyHover={highlightCommits}
						selection={selection}
						select={select}
						className={styles.unassignedChanges}
					/>
				</div>

				<div className={styles.headInfo}>
					<div className={styles.stackLanes}>
						{headInfo.stacks.map((stack) => (
							<div key={stack.id} className={styles.stackLane}>
								<StackC
									commitDetailsSelection={commitDetailsSelection}
									highlightedCommitIds={highlightedCommitIds}
									onDependencyHover={highlightCommits}
									projectId={project.id}
									selection={selection}
									select={select}
									selectCommitDetails={selectCommitDetails}
									stack={stack}
								/>
							</div>
						))}
					</div>

					{commonBaseCommitId !== undefined && (
						<TearOffBranchTarget className={styles.commonBaseCommit}>
							{shortCommitId(commonBaseCommitId)} (common base commit)
						</TearOffBranchTarget>
					)}
				</div>

				<TearOffBranchTarget className={styles.emptyLane} />
			</div>

			<PositionedShortcutBar mode={getShortcutBarMode({ selection, commitDetailsSelection })} />
		</ProjectPreviewLayout>
	);
};

export const Route = createFileRoute("/project/$id/workspace")({
	component: ProjectPage,
});
