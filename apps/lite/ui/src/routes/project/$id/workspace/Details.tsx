import uiStyles from "#ui/components/ui.module.css";
import { SuspenseQuery } from "@suspensive/react-query";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeRefName } from "#ui/api/ref-name.ts";
import { commitBody, commitTitle, shortCommitId } from "#ui/commit.ts";
import { branchOperand, changesSectionOperand, commitOperand, type Operand } from "#ui/operands.ts";
import {
	projectActions,
	selectProjectFilesVisible,
	selectProjectSelectionFiles,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { Tooltip } from "@base-ui/react";
import type { DiffHunk, TreeChange } from "@gitbutler/but-sdk";
import { parsePatchFiles } from "@pierre/diffs";
import { CodeView, type CodeViewDiffItem } from "@pierre/diffs/react";
import { useSuspenseQueries } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Hash, Match } from "effect";
import { ComponentProps, FC, Suspense, useDeferredValue } from "react";
import styles from "./Details.module.css";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { SelectionScope } from "#ui/selection-scopes.ts";
import {
	BranchFilesTree,
	ChangesFilesTree,
	CommitFilesTree,
} from "#ui/routes/project/$id/workspace/FilesTree.tsx";

const lineEndingForDiff = (diff: string): string => (diff.includes("\r\n") ? "\r\n" : "\n");

const patchHeaderForChange = (change: TreeChange, lineEnding: string): string =>
	Match.value(change.status).pipe(
		Match.when(
			{ type: "Addition" },
			() =>
				[
					`diff --git a/${change.path} b/${change.path}`,
					"new file mode 100644",
					"--- /dev/null",
					`+++ b/${change.path}`,
				].join(lineEnding) + lineEnding,
		),

		Match.when(
			{ type: "Deletion" },
			() =>
				[
					`diff --git a/${change.path} b/${change.path}`,
					"deleted file mode 100644",
					`--- a/${change.path}`,
					"+++ /dev/null",
				].join(lineEnding) + lineEnding,
		),

		Match.when(
			{ type: "Modification" },
			() =>
				[
					`diff --git a/${change.path} b/${change.path}`,
					`--- a/${change.path}`,
					`+++ b/${change.path}`,
				].join(lineEnding) + lineEnding,
		),

		Match.when(
			{ type: "Rename" },
			({ subject }) =>
				[
					`diff --git a/${subject.previousPath} b/${change.path}`,
					"similarity index 99%",
					`rename from ${subject.previousPath}`,
					`rename to ${change.path}`,
					`--- a/${subject.previousPath}`,
					`+++ b/${change.path}`,
				].join(lineEnding) + lineEnding,
		),

		Match.exhaustive,
	);

const mkCodeViewItem = (
	change: TreeChange,
	changesetKey: string,
	hunks: Array<DiffHunk>,
): CodeViewDiffItem | null => {
	const lineEnding = lineEndingForDiff(hunks[0]?.diff ?? "");
	const header = patchHeaderForChange(change, lineEnding);
	const combinedFilePatch = [header, ...hunks.map((hunk) => hunk.diff)].join(lineEnding);
	const parsed = parsePatchFiles(combinedFilePatch);

	return {
		type: "diff",
		id: `${changesetKey}:${change.path}`,
		version: Hash.string(combinedFilePatch),
		// oxlint-disable-next-line typescript/no-non-null-assertion: There should always be exactly one result given our one parsed hunk.
		fileDiff: parsed[0]!.files[0]!,
	};
};

const ChangesFileDiffList: FC<{
	changes: Array<TreeChange>;
	changesetKey: string;
	projectId: string;
}> = ({ changes, changesetKey, projectId }) => {
	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const items = Array.zip(changes, treeChangeDiffs).flatMap(
		([change, mdiff]) =>
			Match.value(mdiff).pipe(
				Match.when(
					{ type: "Patch" },
					(patch) => mkCodeViewItem(change, changesetKey, patch.subject.hunks) ?? [],
				),
				Match.when({ type: "Binary" }, () => mkCodeViewItem(change, changesetKey, [])),
				Match.orElse(() => []),
			) ?? [],
	);

	return items.length === 0 ? (
		<p className="text-13">No changes.</p>
	) : (
		<CodeView
			className={classes(styles.diffContentsVirtualizer, uiStyles.scrollerWithSeparator)}
			items={items}
			options={{
				diffStyle: "unified",
				themeType: "system",
				layout: {
					paddingTop: 0,
					paddingBottom: 0,
					gap: 10,
				},
			}}
		/>
	);
};

const Header: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ stackId, branchRef }) => {
				const decodedBranchRef = decodeRefName(branchRef);

				return (
					<SuspenseQuery
						{...branchDetailsQueryOptions({
							projectId,
							// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
							branchName: decodedBranchRef.replace(/^refs\/heads\//, ""),
							remote: null,
						})}
					>
						{({ data: branchDetails }) => (
							<OperationSourceC
								projectId={projectId}
								selectionScope="outline"
								source={branchOperand({ stackId, branchRef })}
								render={<header className={styles.header} />}
							>
								<h3 className={classes("text-14", "text-semibold")}>{branchDetails.name}</h3>
								{branchDetails.prNumber != null && (
									<div className={classes("text-13", "text-bold", styles.pr)}>
										PR #{branchDetails.prNumber}
									</div>
								)}
							</OperationSourceC>
						)}
					</SuspenseQuery>
				);
			},
			ChangesSection: () => (
				<OperationSourceC
					projectId={projectId}
					selectionScope="outline"
					source={changesSectionOperand}
					render={<header className={styles.header} />}
				>
					<h3 className={classes("text-14", "text-semibold")}>Changes</h3>
				</OperationSourceC>
			),
			File: () => null,
			Commit: ({ commitId, stackId }) => {
				const source = commitOperand({ stackId, commitId });

				return (
					<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
						{({ data: commitDetails }) => (
							<OperationSourceC
								projectId={projectId}
								selectionScope="outline"
								source={source}
								render={<header className={styles.header} />}
							>
								<Icon name="commit" />
								<h3 className={classes("text-14", "text-semibold")}>
									{commitTitle(commitDetails.commit.message)}
									{commitDetails.commit.hasConflicts && " ⚠️"}
								</h3>
								<span className={classes("text-13", styles.commitMeta)}>
									#{shortCommitId(commitDetails.commit.id)}
								</span>
							</OperationSourceC>
						)}
					</SuspenseQuery>
				);
			},
			Hunk: () => null,
		}),
	);

const FilesToggle: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				className={getButtonClassName({})}
				aria-pressed={filesVisible}
				onClick={() => dispatch(projectActions.toggleFiles({ projectId }))}
			>
				Files
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.toggleFiles.hotkey} />}>
						{workspaceHotkeys.toggleFiles.meta.name}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const CommitDetailsContent: FC<{
	projectId: string;
	commitId: string;
}> = ({ projectId, commitId }) => (
	<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
		{({ data: commitDetails }) => {
			const fmtDate = new Intl.DateTimeFormat(undefined, {
				day: "2-digit",
				month: "2-digit",
				year: "numeric",
				hour: "2-digit",
				minute: "2-digit",
				hour12: false,
			}).format(commitDetails.commit.createdAt);

			const body = commitBody(commitDetails.commit.message);

			return (
				<>
					{body !== undefined && (
						<p className={classes("text-monospace", "text-body", styles.commitMessageBody)}>
							{body}
						</p>
					)}
					<div className={styles.commitDetailsMeta}>
						<img
							src={commitDetails.commit.author.gravatarUrl}
							className={styles.avatar}
							alt="Commit author avatar"
						/>
						<div className={classes("text-13", styles.author)}>
							<span title={commitDetails.commit.author.email}>
								{commitDetails.commit.author.name}
							</span>{" "}
							at {fmtDate}
						</div>
						<div className={classes("text-13", styles.commitMeta)}>
							{shortCommitId(commitDetails.commit.changeId)} (
							{shortCommitId(commitDetails.commit.id)})
						</div>
					</div>
				</>
			);
		}}
	</SuspenseQuery>
);

const DiffContents: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef }) => (
				<SuspenseQuery {...branchDiffQueryOptions({ projectId, branch: decodeRefName(branchRef) })}>
					{({ data: branchDiff }) => (
						<ChangesFileDiffList
							changes={branchDiff.changes}
							changesetKey={decodeRefName(branchRef)}
							projectId={projectId}
						/>
					)}
				</SuspenseQuery>
			),
			ChangesSection: () => (
				<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
					{({ data: worktreeChanges }) => (
						<ChangesFileDiffList
							changes={worktreeChanges.changes}
							changesetKey="changes"
							projectId={projectId}
						/>
					)}
				</SuspenseQuery>
			),
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: () => (
							<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
								{({ data: worktreeChanges }) => {
									const selectedChange = worktreeChanges.changes.find(
										(candidate) => candidate.path === path,
									);

									return (
										<ChangesFileDiffList
											changes={selectedChange ? [selectedChange] : worktreeChanges.changes}
											changesetKey="changes"
											projectId={projectId}
										/>
									);
								}}
							</SuspenseQuery>
						),

						Branch: ({ branchRef }) => (
							<SuspenseQuery
								{...branchDiffQueryOptions({
									projectId,
									branch: decodeRefName(branchRef),
								})}
							>
								{({ data: branchDiff }) => {
									const selectedChange = branchDiff.changes.find(
										(candidate) => candidate.path === path,
									);

									return (
										<ChangesFileDiffList
											changes={selectedChange ? [selectedChange] : branchDiff.changes}
											changesetKey={decodeRefName(branchRef)}
											projectId={projectId}
										/>
									);
								}}
							</SuspenseQuery>
						),
						Commit: ({ commitId }) => (
							<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
								{({ data: commitDetails }) => {
									const selectedChange = commitDetails.changes.find(
										(candidate) => candidate.path === path,
									);
									if (!selectedChange) return null;

									return (
										<ChangesFileDiffList
											changes={[selectedChange]}
											changesetKey={commitId}
											projectId={projectId}
										/>
									);
								}}
							</SuspenseQuery>
						),
					}),
				),
			Commit: ({ commitId }) => (
				<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
					{({ data: commitDetails }) => (
						<ChangesFileDiffList
							changes={commitDetails.changes}
							changesetKey={commitId}
							projectId={projectId}
						/>
					)}
				</SuspenseQuery>
			),
			Hunk: () => null,
		}),
	);

const FilesTree: FC<ComponentProps<"div">> = (props) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const outlineSelection = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);

	return (
		<Suspense
			fallback={
				<div {...props} className={classes(props.className, "text-13")}>
					Loading files…
				</div>
			}
		>
			{Match.value(outlineSelection).pipe(
				Match.tag("Commit", (commit) => (
					<SuspenseQuery
						{...commitDetailsWithLineStatsQueryOptions({
							projectId,
							commitId: commit.commitId,
						})}
					>
						{({ data: commitDetails }) => (
							<CommitFilesTree
								{...props}
								projectId={projectId}
								commit={commit}
								commitDetails={commitDetails}
							/>
						)}
					</SuspenseQuery>
				)),
				Match.tag("ChangesSection", () => (
					<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
						{({ data: worktreeChanges }) => (
							<ChangesFilesTree
								{...props}
								projectId={projectId}
								worktreeChanges={worktreeChanges}
							/>
						)}
					</SuspenseQuery>
				)),
				Match.tag("Branch", ({ stackId, branchRef }) => (
					<SuspenseQuery
						{...branchDiffQueryOptions({ projectId, branch: decodeRefName(branchRef) })}
					>
						{({ data: branchDiff }) => (
							<BranchFilesTree
								{...props}
								projectId={projectId}
								stackId={stackId}
								branchRef={branchRef}
								branchDiff={branchDiff}
							/>
						)}
					</SuspenseQuery>
				)),
				Match.orElse(() => <div {...props} />),
			)}
		</Suspense>
	);
};

export const Details: FC<ComponentProps<"div">> = (props) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));
	const urgentOutlineSelection = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);
	const outlineSelection = useDeferredValue(urgentOutlineSelection);
	const urgentFilesSelection = useAppSelector((state) =>
		selectProjectSelectionFiles(state, projectId),
	);
	const filesSelection = useDeferredValue(urgentFilesSelection);

	if (outlineSelection._tag === "Stack") return;

	return (
		<div
			{...props}
			className={classes(props.className, styles.container)}
			style={{ opacity: urgentOutlineSelection !== outlineSelection ? 0.5 : 1 }}
		>
			<div className={styles.headerWrap}>
				<Suspense fallback={<p className="text-13">Loading details…</p>}>
					<Header projectId={projectId} selection={outlineSelection} />

					{outlineSelection._tag === "Commit" && (
						<CommitDetailsContent projectId={projectId} commitId={outlineSelection.commitId} />
					)}
				</Suspense>

				<div>
					<FilesToggle />
				</div>
			</div>

			<div className={classes(styles.diff, filesVisible && styles.diffWithFiles)}>
				{filesVisible && (
					<FilesTree
						id={"files" satisfies SelectionScope}
						data-selection-scope
						tabIndex={0}
						className={classes(styles.diffFiles, uiStyles.scrollerWithSeparator)}
					/>
				)}

				<div
					id={"diff" satisfies SelectionScope}
					data-selection-scope
					// oxlint-disable-next-line jsx_a11y/no-noninteractive-tabindex -- Revisit this when we add hunk/line selection.
					tabIndex={0}
					className={styles.diffContents}
					style={{ opacity: urgentFilesSelection !== filesSelection ? 0.5 : 1 }}
				>
					<Suspense fallback={<p className="text-13">Loading diff…</p>}>
						<DiffContents projectId={projectId} selection={filesSelection} />
					</Suspense>
				</div>
			</div>
		</div>
	);
};
