import { commitDetailsWithLineStatsQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import { ExpandCollapseIcon, MenuTriggerIcon } from "#ui/components/icons.tsx";
import { ContextMenu, Menu } from "@base-ui/react";
import { BranchDetails, BranchListing, Commit, DiffHunk } from "@gitbutler/but-sdk";
import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Match } from "effect";
import { ComponentProps, FC, Suspense, useTransition } from "react";
import useLocalStorageState from "use-local-storage-state";
import styles from "./route.module.css";

import { applyBranchMutationOptions, unapplyStackMutationOptions } from "#ui/api/mutations.ts";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { CheckIcon } from "#ui/components/icons.tsx";
import { ProjectPreviewLayout } from "#ui/routes/project/$id/ProjectPreviewLayout.tsx";
import {
	CommitDetails,
	CommitLabel,
	CommitsList,
	FileButton,
	FileDiff,
	formatHunkHeader,
	HunkDiff,
} from "#ui/routes/project/$id/shared.tsx";
import sharedStyles from "../shared.module.css";
import {
	getDefaultSelection,
	isBranchSelected,
	isBranchSelectedWithin,
	isCommitExpanded,
	isCommitFileSelected,
	isCommitSelected,
	normalizeBranchSelection,
	Selection,
	toggleBranchSelection,
	toggleCommitFileSelection,
	toggleCommitSelection,
} from "./Selection.ts";

const isValidCommit = (commitId: string, branchDetails: BranchDetails): boolean => {
	const commitIds = new Set(branchDetails.commits.map((commit) => commit.id));
	return commitIds.has(commitId);
};

const getBranchRef = (branch: BranchListing): string | null => {
	if (branch.hasLocal) return `refs/heads/${branch.name}`;
	const remote = branch.remotes[0];
	if (remote === undefined) return null;
	return `refs/remotes/${remote}/${branch.name}`;
};

const BranchMenuPopup: FC<{
	branch: BranchListing;
	projectId: string;
	parts: typeof Menu | typeof ContextMenu;
}> = ({ branch, projectId, parts }) => {
	const { Popup, Item } = parts;
	const applyBranch = useMutation(applyBranchMutationOptions);
	const unapplyStack = useMutation(unapplyStackMutationOptions);
	const ref = getBranchRef(branch);
	const stackId = branch.stack?.id;

	return (
		<Popup className={sharedStyles.menuPopup}>
			{!branch.stack?.inWorkspace ? (
				<Item
					className={sharedStyles.menuItem}
					disabled={ref === null}
					onClick={() => {
						if (ref === null) return;
						applyBranch.mutate({
							projectId,
							existingBranch: ref,
						});
					}}
				>
					Apply branch
				</Item>
			) : (
				<Item
					className={sharedStyles.menuItem}
					disabled={stackId === undefined}
					onClick={() => {
						if (stackId === undefined) return;
						unapplyStack.mutate({ projectId, stackId });
					}}
				>
					Unapply stack
				</Item>
			)}
		</Popup>
	);
};

const BranchApplyToggle: FC<{
	branch: BranchListing;
	projectId: string;
}> = ({ branch, projectId }) => {
	const applyBranch = useMutation(applyBranchMutationOptions);
	const unapplyStack = useMutation(unapplyStackMutationOptions);
	const ref = getBranchRef(branch);
	const stackId = branch.stack?.id;
	const isApplied = branch.stack?.inWorkspace ?? false;

	return isApplied ? (
		<button
			type="button"
			className={sharedStyles.rowAction}
			disabled={stackId === undefined}
			aria-label={`Unapply branch ${branch.name}`}
			onClick={() => {
				if (stackId === undefined) return;
				unapplyStack.mutate({ projectId, stackId });
			}}
		>
			<CheckIcon />
		</button>
	) : (
		<button
			type="button"
			className={classes(sharedStyles.rowAction, styles.branchApplyButtonInactive)}
			disabled={ref === null}
			aria-label={`Apply branch ${branch.name}`}
			onClick={() => {
				if (ref === null) return;
				applyBranch.mutate({
					projectId,
					existingBranch: ref,
				});
			}}
		>
			<CheckIcon />
		</button>
	);
};

const BranchRow: FC<
	{
		projectId: string;
		branch: BranchListing;
		selection: Selection | null;
		select: (selection: Selection | null) => void;
	} & ComponentProps<"div">
> = ({ projectId, branch, selection, select, className, ...restProps }) => {
	const isSelected = isBranchSelected(selection, branch.name);
	const isSelectedWithin = isBranchSelectedWithin(selection, branch.name);

	return (
		<div
			{...restProps}
			className={classes(
				sharedStyles.row,
				styles.branchRow,
				isSelected || isSelectedWithin ? sharedStyles.selected : undefined,
				className,
			)}
		>
			<ContextMenu.Root>
				<ContextMenu.Trigger
					render={
						<button
							type="button"
							className={styles.branchButton}
							onClick={() => {
								select(toggleBranchSelection(selection, branch.name));
							}}
						>
							{branch.name}
							{branch.stack?.branches && branch.stack.branches.length > 1 && (
								<> (+{branch.stack.branches.length - 1} more)</>
							)}
						</button>
					}
				/>
				<ContextMenu.Portal>
					<ContextMenu.Positioner>
						<BranchMenuPopup branch={branch} projectId={projectId} parts={ContextMenu} />
					</ContextMenu.Positioner>
				</ContextMenu.Portal>
			</ContextMenu.Root>
			<BranchApplyToggle branch={branch} projectId={projectId} />
			<Menu.Root>
				<Menu.Trigger className={sharedStyles.rowAction} aria-label={`Branch ${branch.name} menu`}>
					<MenuTriggerIcon />
				</Menu.Trigger>
				<Menu.Portal>
					<Menu.Positioner align="end">
						<BranchMenuPopup branch={branch} projectId={projectId} parts={Menu} />
					</Menu.Positioner>
				</Menu.Portal>
			</Menu.Root>
		</div>
	);
};

const CommitRow: FC<{
	branchName: string;
	commit: Commit;
	projectId: string;
	selection: Selection | null;
	select: (selection: Selection | null) => void;
	isHighlighted: boolean;
}> = ({ branchName, commit, projectId, selection, select, isHighlighted }) => {
	const [isExpandPending, startExpandTransition] = useTransition();
	const queryClient = useQueryClient();
	const isSelected = isCommitSelected(selection, branchName, commit.id);
	const isExpanded = isCommitExpanded(selection, branchName, commit.id);

	return (
		<div
			className={classes(
				sharedStyles.row,
				sharedStyles.commitRow,
				isSelected || isExpanded ? sharedStyles.selected : undefined,
				isHighlighted && sharedStyles.highlighted,
			)}
			style={{ ...(isExpandPending && { opacity: 0.5 }) }}
			aria-busy={isExpandPending}
		>
			<button
				type="button"
				className={sharedStyles.commitButton}
				onClick={() => {
					select(toggleCommitSelection(selection, branchName, commit.id));
				}}
			>
				<CommitLabel commit={commit} />
			</button>
			<button
				className={sharedStyles.rowAction}
				type="button"
				onClick={() => {
					startExpandTransition(async () => {
						if (isExpanded) {
							select({ _tag: "Commit", branchName, commitId: commit.id, isExpanded: false });
							return;
						}

						const commitDetails = await queryClient.ensureQueryData(
							commitDetailsWithLineStatsQueryOptions({ projectId, commitId: commit.id }),
						);
						const firstPath = commitDetails.changes[0]?.path;

						select(
							firstPath !== undefined
								? {
										_tag: "Commit",
										branchName,
										commitId: commit.id,
										path: firstPath,
										isExpanded: true,
									}
								: { _tag: "Commit", branchName, commitId: commit.id, isExpanded: true },
						);
					});
				}}
				aria-expanded={isExpanded}
				aria-label={isExpanded ? "Collapse commit" : "Expand commit"}
			>
				<ExpandCollapseIcon isExpanded={isExpanded} />
			</button>
		</div>
	);
};

const CommitC: FC<{
	branchName: string;
	commit: Commit;
	projectId: string;
	selection: Selection | null;
	select: (selection: Selection | null) => void;
}> = ({ branchName, commit, projectId, selection, select }) => (
	<div>
		<CommitRow
			branchName={branchName}
			commit={commit}
			projectId={projectId}
			selection={selection}
			select={select}
			isHighlighted={false}
		/>
		{isCommitExpanded(selection, branchName, commit.id) && (
			<div className={sharedStyles.commitDetails}>
				<Suspense fallback={<div>Loading changed details…</div>}>
					<CommitDetails
						projectId={projectId}
						commitId={commit.id}
						renderFile={(change) => (
							<div
								className={classes(
									sharedStyles.row,
									sharedStyles.fileRow,
									isCommitFileSelected(selection, branchName, commit.id, change.path) &&
										sharedStyles.selectedFile,
								)}
							>
								<FileButton
									change={change}
									toggleSelect={() => {
										select(
											toggleCommitFileSelection(selection, branchName, commit.id, change.path),
										);
									}}
								/>
							</div>
						)}
					/>
				</Suspense>
			</div>
		)}
	</div>
);

const BranchDetailsC: FC<{
	branchName: string;
	projectId: string;
	remote: string | null;
	selection: Selection | null;
	select: (selection: Selection | null) => void;
}> = ({ branchName, projectId, remote, selection, select }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);

	return (
		<CommitsList commits={branchDetails.commits}>
			{(commit) => (
				<CommitC
					branchName={branchName}
					commit={commit}
					projectId={projectId}
					selection={selection}
					select={select}
				/>
			)}
		</CommitsList>
	);
};

const Hunk: FC<{
	hunk: DiffHunk;
}> = ({ hunk }) => (
	<div>
		<div className={styles.hunkHeaderRow}>{formatHunkHeader(hunk)}</div>
		<HunkDiff diff={hunk.diff} />
	</div>
);

const CommitFileDiff: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	commitId: string;
	path: string;
}> = ({ projectId, branchName, remote, commitId, path }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	if (!isValidCommit(commitId, branchDetails)) return null;

	const change = data.changes.find((candidate) => candidate.path === path);

	if (!change) return null;

	return (
		<FileDiff projectId={projectId} change={change} renderHunk={(hunk) => <Hunk hunk={hunk} />} />
	);
};

const CommitDiff: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	commitId: string;
}> = ({ projectId, branchName, remote, commitId }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	if (!isValidCommit(commitId, branchDetails)) return null;

	return data.changes.length === 0 ? (
		<div>No file changes.</div>
	) : (
		<ul>
			{data.changes.map((change) => (
				<li key={change.path}>
					<h5>{change.path}</h5>
					<FileDiff
						projectId={projectId}
						change={change}
						renderHunk={(hunk) => <Hunk hunk={hunk} />}
					/>
				</li>
			))}
		</ul>
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
							<h5>{change.path}</h5>
							<FileDiff
								projectId={projectId}
								change={change}
								renderHunk={(hunk) => <Hunk hunk={hunk} />}
							/>
						</li>
					))}
				</ul>
			)}
		</>
	);
};

const Preview: FC<{
	projectId: string;
	selection: Selection;
	remote: string | null;
	selectedBranchRef: string | null;
}> = ({ projectId, selection, remote, selectedBranchRef }) =>
	Match.value(selection).pipe(
		Match.tag("Branch", ({ branchName }) =>
			selectedBranchRef !== null ? (
				<ShowBranch projectId={projectId} branch={selectedBranchRef} branchName={branchName} />
			) : (
				<div>No branch diff available.</div>
			),
		),
		Match.tag("Commit", ({ branchName, commitId, path }) =>
			path === undefined ? (
				<CommitDiff
					projectId={projectId}
					branchName={branchName}
					remote={remote}
					commitId={commitId}
				/>
			) : (
				<CommitFileDiff
					projectId={projectId}
					branchName={branchName}
					remote={remote}
					commitId={commitId}
					path={path}
				/>
			),
		),
		Match.exhaustive,
	);

const ProjectBranchesPage: FC = () => {
	const { id: projectId } = Route.useParams();

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());
	const project = projects.find((project) => project.id === projectId);
	const { data: branches } = useSuspenseQuery(listBranchesQueryOptions(projectId));

	const sortedBranches = branches.slice().sort((a, b) => a.name.localeCompare(b.name));
	const [_selection, select] = useLocalStorageState<Selection | null>(
		`project:${projectId}:branches:selection`,
		{ defaultValue: null },
	);
	const selection =
		(_selection ? normalizeBranchSelection(_selection, sortedBranches) : null) ??
		getDefaultSelection(sortedBranches);
	const selectedBranch = sortedBranches.find((branch) => branch.name === selection?.branchName);
	const selectedRemote =
		selectedBranch && !selectedBranch.hasLocal ? selectedBranch.remotes[0] : null;

	if (!project) return <p>Project not found.</p>;

	return sortedBranches.length === 0 ? (
		<p>No branches found.</p>
	) : (
		<ProjectPreviewLayout
			projectId={projectId}
			preview={
				selection && (
					<Suspense fallback={<div>Loading diff…</div>}>
						<Preview
							projectId={projectId}
							selection={selection}
							selectedBranchRef={selectedBranch ? getBranchRef(selectedBranch) : null}
							remote={selectedRemote ?? null}
						/>
					</Suspense>
				)
			}
		>
			<div className={sharedStyles.lanes}>
				<ul className={styles.branchesListLane}>
					{sortedBranches.map((branch) => (
						<li key={branch.name}>
							<BranchRow
								projectId={projectId}
								branch={branch}
								selection={selection}
								select={select}
							/>
						</li>
					))}
				</ul>

				{selectedBranch?.name != null && (
					<div className={styles.branchDetailsLane}>
						<Suspense fallback={<div>Loading branch details…</div>}>
							<BranchDetailsC
								branchName={selectedBranch.name}
								projectId={projectId}
								remote={selectedRemote ?? null}
								selection={selection}
								select={select}
							/>
						</Suspense>
					</div>
				)}
			</div>
		</ProjectPreviewLayout>
	);
};

export const Route = createFileRoute("/project/$id/branches")({
	component: ProjectBranchesPage,
});
