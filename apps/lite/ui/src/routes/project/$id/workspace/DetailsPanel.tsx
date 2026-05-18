import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeRefName } from "#ui/api/ref-name.ts";
import { commitTitle } from "#ui/commit.ts";
import {
	formatHunkHeader,
	getDependencyCommitIds,
	getHunkDependencyDiffsByPath,
	type HunkDependencyDiff,
} from "#ui/hunk.ts";
import {
	branchFileParent,
	changesFileParent,
	commitFileParent,
	fileOperand,
	hunkOperand,
	type FileParent,
	type Operand,
} from "#ui/operands.ts";
import { pointerTransferOperationMode } from "#ui/outline/mode.ts";
import {
	projectActions,
	selectProjectOutlineModeState,
	selectProjectSelectionFiles,
} from "#ui/projects/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import operationSourceStyles from "#ui/routes/project/$id/workspace/OperationSourceC.module.css";
import { OperationSourceLabel } from "#ui/routes/project/$id/workspace/OperationSourceLabel.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/ui/classes.ts";
import {
	draggable,
	type ElementGetFeedbackArgs,
} from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import { DiffHunk, TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { parsePatchFiles, type FileDiffMetadata } from "@pierre/diffs";
import { FileDiff as PFileDiff, Virtualizer } from "@pierre/diffs/react";
import { useQuery, useSuspenseQueries, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Match } from "effect";
import { FC, Suspense, useCallback, useDeferredValue, useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import { Panel, PanelProps } from "react-resizable-panels";
import styles from "./DetailsPanel.module.css";

const ensureTrailingLineEnding = (value: string, lineEnding: string): string =>
	value.endsWith("\n") ? value : `${value}${lineEnding}`;

const patchForChange = (change: TreeChange, hunks: Array<DiffHunk>): string => {
	const lineEnding = hunks.find((hunk) => hunk.diff.includes("\r\n")) ? "\r\n" : "\n";
	return `${patchHeaderForChange(change, lineEnding)}${hunks
		.map((hunk) => ensureTrailingLineEnding(hunk.diff, lineEnding))
		.join("")}`;
};

const patchHeaderForChange = (change: TreeChange, lineEnding: string): string =>
	Match.value(change.status).pipe(
		Match.when(
			{ type: "Addition" },
			() => `--- /dev/null${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.when(
			{ type: "Deletion" },
			() => `--- ${change.path}${lineEnding}+++ /dev/null${lineEnding}`,
		),
		Match.when(
			{ type: "Modification" },
			() => `--- ${change.path}${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.when(
			{ type: "Rename" },
			({ subject }) => `--- ${subject.previousPath}${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.exhaustive,
	);

const HUNK_DRAG_HANDLE_SELECTOR = "[data-gitbutler-hunk-drag-handle]";

const styleHunkDragHandle = (handle: HTMLButtonElement): void => {
	handle.style.position = "absolute";
	handle.style.insetBlockStart = "50%";
	handle.style.insetInlineStart = "2px";
	handle.style.zIndex = "1";
	handle.style.width = "14px";
	handle.style.height = "16px";
	handle.style.padding = "0";
	handle.style.border = "1px solid var(--border-2)";
	handle.style.borderRadius = "3px";
	handle.style.background = "var(--bg-1)";
	handle.style.boxShadow = "0 1px 2px rgb(0 0 0 / 12%)";
	handle.style.color = "inherit";
	handle.style.cursor = "grab";
	handle.style.font = "inherit";
	handle.style.fontSize = "9px";
	handle.style.lineHeight = "1";
	handle.style.opacity = "0.8";
	handle.style.transform = "translateY(-50%)";
};

const createHunkDragHandle = (hunkIndex: number, label: string): HTMLButtonElement => {
	const handle = document.createElement("button");
	handle.type = "button";
	handle.draggable = true;
	handle.dataset.gitbutlerHunkDragHandle = "";
	handle.dataset.hunkIndex = `${hunkIndex}`;
	handle.textContent = "::";
	handle.ariaLabel = label;
	handle.title = label;
	styleHunkDragHandle(handle);
	return handle;
};

const FileDiffWithHunkHandles: FC<{
	projectId: string;
	change: TreeChange;
	fileParent: FileParent;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
	fileDiff: FileDiffMetadata;
	hunks: Array<DiffHunk>;
	isResultOfBinaryToTextConversion: boolean;
}> = ({
	projectId,
	change,
	fileParent,
	hunkDependencyDiffs,
	fileDiff,
	hunks,
	isResultOfBinaryToTextConversion,
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const dispatch = useAppDispatch();
	const handleSources = useRef(new WeakMap<HTMLElement, Operand>());
	const hostDragCleanup = useRef<{ host: HTMLElement; key: string; cleanup: () => void } | null>(
		null,
	);

	const canDrag = useCallback(
		() => outlineMode._tag !== "RenameBranch" && outlineMode._tag !== "RewordCommit",
		[outlineMode._tag],
	);
	const onGenerateDragPreview = useCallback(
		({
			nativeSetDragImage,
			source,
		}: {
			nativeSetDragImage: DataTransfer["setDragImage"] | null;
			source: Operand;
		}) => {
			setCustomNativeDragPreview({
				nativeSetDragImage,
				getOffset: centerUnderPointer,
				render: ({ container }) => {
					if (!headInfo) return;
					const root = createRoot(container);
					root.render(
						<div className={operationSourceStyles.dragPreview}>
							<OperationSourceLabel source={source} headInfo={headInfo} />
						</div>,
					);
					return () => {
						root.unmount();
					};
				},
			});
		},
		[headInfo],
	);
	const onDragStart = useCallback(
		(source: Operand) => {
			dispatch(projectActions.selectFiles({ projectId, selection: source }));
			dispatch(
				projectActions.enterTransferMode({
					projectId,
					mode: pointerTransferOperationMode({
						source,
						operationType: null,
					}),
				}),
			);
		},
		[dispatch, projectId],
	);
	const onDrop = useCallback(
		(location: { current: { dropTargets: Array<unknown> } }) => {
			if (location.current.dropTargets.length > 0) return;

			dispatch(projectActions.cancelMode({ projectId }));
		},
		[dispatch, projectId],
	);
	const hostDragKey = `${outlineMode._tag}:${headInfo ? "ready" : "pending"}`;
	const sourceFromDragInput = useCallback(
		(host: HTMLElement, input: ElementGetFeedbackArgs["input"]): Operand | null => {
			const shadowRoot = host.shadowRoot;
			if (!shadowRoot) return null;

			const element = shadowRoot.elementFromPoint(input.clientX, input.clientY);
			const handle = element?.closest<HTMLElement>(HUNK_DRAG_HANDLE_SELECTOR);
			if (!handle) return null;

			return handleSources.current.get(handle) ?? null;
		},
		[],
	);
	const ensureHostDraggable = useCallback(
		(host: HTMLElement) => {
			if (hostDragCleanup.current?.host === host && hostDragCleanup.current.key === hostDragKey)
				return;

			hostDragCleanup.current?.cleanup();
			hostDragCleanup.current = {
				host,
				key: hostDragKey,
				cleanup: draggable({
					element: host,
					canDrag: ({ input }) => canDrag() && sourceFromDragInput(host, input) !== null,
					getInitialData: ({ input }) => {
						const source = sourceFromDragInput(host, input);
						return source ? { source } : {};
					},
					onGenerateDragPreview: ({ location, nativeSetDragImage }) => {
						const source = sourceFromDragInput(host, location.initial.input);
						if (!source) return;

						onGenerateDragPreview({ nativeSetDragImage, source });
					},
					onDragStart: ({ location }) => {
						const source = sourceFromDragInput(host, location.initial.input);
						if (!source) return;

						onDragStart(source);
					},
					onDrop: ({ location }) => onDrop(location),
				}),
			};
		},
		[canDrag, hostDragKey, onDragStart, onDrop, onGenerateDragPreview, sourceFromDragInput],
	);

	const updateHunkDragHandles = useCallback(
		(host: HTMLElement) => {
			const shadowRoot = host.shadowRoot;
			if (!shadowRoot) return;

			ensureHostDraggable(host);

			const liveHandles = new Set<HTMLElement>();

			for (const [hunkIndex, fileDiffHunk] of fileDiff.hunks.entries()) {
				const sdkHunk = hunks[hunkIndex];
				if (!sdkHunk) continue;

				const lineIndex = `${fileDiffHunk.unifiedLineStart},${fileDiffHunk.splitLineStart}`;
				const gutterCell = shadowRoot.querySelector<HTMLElement>(
					`[data-line-index="${lineIndex}"][data-column-number]`,
				);
				if (!gutterCell) continue;

				gutterCell.style.position = "relative";
				gutterCell.style.overflow = "visible";

				const source = hunkOperand({
					parent: { parent: fileParent, path: change.path },
					hunkHeader: sdkHunk,
					isResultOfBinaryToTextConversion,
				});
				const dependencyCommitIds =
					fileParent._tag === "Changes" && hunkDependencyDiffs
						? getDependencyCommitIds({ hunk: sdkHunk, hunkDependencyDiffs })
						: undefined;
				const hunkLabel = formatHunkHeader(sdkHunk);
				const dependencyLabel = dependencyCommitIds
					? `, ${dependencyCommitIds.length} dependent commit${
							dependencyCommitIds.length === 1 ? "" : "s"
						}`
					: "";
				const label = `Drag hunk ${hunkLabel}${dependencyLabel}`;
				const sourceKey = JSON.stringify([
					change.path,
					fileParent,
					sdkHunk,
					isResultOfBinaryToTextConversion,
				]);
				let handle = gutterCell.querySelector<HTMLButtonElement>(HUNK_DRAG_HANDLE_SELECTOR);

				if (handle?.dataset.hunkDragSourceKey !== sourceKey) {
					handle?.remove();
					handle = createHunkDragHandle(hunkIndex, label);
					handle.dataset.hunkDragSourceKey = sourceKey;
					gutterCell.appendChild(handle);
				} else {
					handle.dataset.hunkIndex = `${hunkIndex}`;
					handle.ariaLabel = label;
					handle.title = label;
				}

				handleSources.current.set(handle, source);
				liveHandles.add(handle);
			}

			for (const handle of shadowRoot.querySelectorAll<HTMLElement>(HUNK_DRAG_HANDLE_SELECTOR))
				if (!liveHandles.has(handle)) handle.remove();
		},
		[
			change.path,
			ensureHostDraggable,
			fileDiff,
			fileParent,
			hunkDependencyDiffs,
			hunks,
			isResultOfBinaryToTextConversion,
		],
	);

	useEffect(
		() => () => {
			hostDragCleanup.current?.cleanup();
			hostDragCleanup.current = null;
		},
		[],
	);

	return (
		<PFileDiff
			fileDiff={fileDiff}
			options={{
				diffStyle: "unified",
				themeType: "system",
				disableFileHeader: true,
				onPostRender: updateHunkDragHandles,
			}}
		/>
	);
};

const FileDiff: FC<{
	projectId: string;
	change: TreeChange;
	fileParent: FileParent;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
	diff: UnifiedPatch | null;
}> = ({ projectId, change, fileParent, hunkDependencyDiffs, diff }) =>
	Match.value(diff).pipe(
		Match.when(null, () => <div>No diff available for this file.</div>),
		Match.when({ type: "Binary" }, () => <div>Binary file (diff not available).</div>),
		Match.when({ type: "TooLarge" }, ({ subject }) => (
			<div>Diff too large ({subject.sizeInBytes} bytes).</div>
		)),
		Match.when({ type: "Patch" }, (patch) => {
			const { hunks } = patch.subject;
			if (hunks.length === 0) return <div>No hunks.</div>;

			const [parsed] = parsePatchFiles(patchForChange(change, hunks));
			const fileDiff = parsed?.files[0];
			if (!fileDiff) return <div>No diff available for this file.</div>;

			return (
				<FileDiffWithHunkHandles
					projectId={projectId}
					change={change}
					fileParent={fileParent}
					hunkDependencyDiffs={hunkDependencyDiffs}
					fileDiff={fileDiff}
					hunks={hunks}
					isResultOfBinaryToTextConversion={patch.subject.isResultOfBinaryToTextConversion}
				/>
			);
		}),
		Match.exhaustive,
	);

const ChangesFileDiffList: FC<{
	changes: Array<TreeChange>;
	projectId: string;
	fileParent: FileParent;
	hunkDependencyDiffsByPath?: Map<string, Array<HunkDependencyDiff>>;
}> = ({ changes, projectId, fileParent, hunkDependencyDiffsByPath }) => {
	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = Array.zip(changes, treeChangeDiffs);

	return changesWithDiffs.length === 0 ? (
		<div>No file changes.</div>
	) : (
		<ul>
			{changesWithDiffs.map(([change, diff]) => {
				const source = fileOperand({ parent: fileParent, path: change.path });

				return (
					<li key={change.path}>
						<OperationSourceC projectId={projectId} selectionScope="files" source={source}>
							<h4>{change.path}</h4>
						</OperationSourceC>
						<FileDiff
							projectId={projectId}
							change={change}
							fileParent={fileParent}
							hunkDependencyDiffs={hunkDependencyDiffsByPath?.get(change.path)}
							diff={diff}
						/>
					</li>
				);
			})}
		</ul>
	);
};

const ChangesDetails: FC<{
	projectId: string;
	selectedPath?: string;
}> = ({ projectId, selectedPath }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);
	const selectedChange =
		selectedPath !== undefined
			? worktreeChanges.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : worktreeChanges.changes;

	return (
		<div>
			<ChangesFileDiffList
				changes={changes}
				fileParent={changesFileParent}
				hunkDependencyDiffsByPath={hunkDependencyDiffsByPath}
				projectId={projectId}
			/>
		</div>
	);
};

const CommitDetails: FC<{
	projectId: string;
	commitId: string;
	selectedPath?: string | null;
	stackId: string;
}> = ({ projectId, commitId, selectedPath, stackId }) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	const fileParent = commitFileParent({ stackId, commitId });

	if (selectedPath === undefined)
		return (
			<div>
				<h3>
					{commitTitle(commitDetails.commit.message)}
					{commitDetails.commit.hasConflicts && " ⚠️"}
				</h3>
				{commitDetails.commit.message.includes("\n") && (
					<p className={styles.commitMessageBody}>
						{commitDetails.commit.message
							.slice(commitDetails.commit.message.indexOf("\n") + 1)
							.trim()}
					</p>
				)}
				<ChangesFileDiffList
					changes={commitDetails.changes}
					fileParent={fileParent}
					projectId={projectId}
				/>
			</div>
		);

	const selectedChange = commitDetails.changes.find((candidate) => candidate.path === selectedPath);
	if (!selectedChange) return null;

	return (
		<div>
			<ChangesFileDiffList
				changes={[selectedChange]}
				fileParent={fileParent}
				projectId={projectId}
			/>
		</div>
	);
};

const BranchDetails: FC<{
	projectId: string;
	branchRef: Array<number>;
	selectedPath?: string;
	stackId: string;
}> = ({ projectId, branchRef, selectedPath, stackId }) => {
	const decodedBranchRef = decodeRefName(branchRef);
	const [{ data: branchDetails }, { data: branchDiff }] = useSuspenseQueries({
		queries: [
			branchDetailsQueryOptions({
				projectId,
				// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
				branchName: decodedBranchRef.replace(/^refs\/heads\//, ""),
				remote: null,
			}),
			branchDiffQueryOptions({ projectId, branch: decodedBranchRef }),
		],
	});

	const selectedChange =
		selectedPath !== undefined
			? branchDiff.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : branchDiff.changes;

	return (
		<div>
			<h3>{branchDetails.name}</h3>
			{branchDetails.prNumber != null && <p>PR #{branchDetails.prNumber}</p>}
			<ChangesFileDiffList
				changes={changes}
				projectId={projectId}
				fileParent={branchFileParent({ stackId, branchRef })}
			/>
		</div>
	);
};

const Details: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef, stackId }) => (
				<BranchDetails projectId={projectId} branchRef={branchRef} stackId={stackId} />
			),
			ChangesSection: () => <ChangesDetails projectId={projectId} />,
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: () => <ChangesDetails projectId={projectId} selectedPath={path} />,
						Branch: ({ branchRef, stackId }) => (
							<BranchDetails
								projectId={projectId}
								branchRef={branchRef}
								selectedPath={path}
								stackId={stackId}
							/>
						),
						Commit: ({ commitId, stackId }) => (
							<CommitDetails
								projectId={projectId}
								commitId={commitId}
								stackId={stackId}
								selectedPath={path}
							/>
						),
					}),
				),
			Commit: ({ commitId, stackId }) => (
				<CommitDetails projectId={projectId} commitId={commitId} stackId={stackId} />
			),
			BaseCommit: () => null,
			Hunk: () => null,
		}),
	);

export const DetailsPanel: FC<
	{
		className?: string;
	} & Omit<PanelProps, "className">
> = ({ className, ...panelProps }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const urgentSelection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	const selection = useDeferredValue(urgentSelection);

	return (
		<Panel
			{...panelProps}
			style={{ ...panelProps.style, opacity: urgentSelection !== selection ? 0.5 : 1 }}
		>
			<Virtualizer className={classes(className, styles.detailsVirtualizer)}>
				<Suspense fallback={<div>Loading details…</div>}>
					<Details projectId={projectId} selection={selection} />
				</Suspense>
			</Virtualizer>
		</Panel>
	);
};
