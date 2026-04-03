import { commitDetailsWithLineStatsQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import { ExpandCollapseIcon, MenuTriggerIcon } from "#ui/components/icons.tsx";
import { ContextMenu, Menu } from "@base-ui/react";
import { BranchDetails, BranchListing, Commit, DiffHunk, TreeChange } from "@gitbutler/but-sdk";
import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Match } from "effect";
import { ComponentProps, FC, Suspense, useState, useTransition } from "react";
import styles from "./route.module.css";

import { applyBranchMutationOptions, unapplyStackMutationOptions } from "#ui/api/mutations.ts";
import {
	branchDetailsQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { CheckIcon, ArrowDownIcon, ArrowUpIcon, AddCircleIcon } from "#ui/components/icons.tsx";
import { ProjectPreviewLayout } from "#ui/routes/project/$id/-ProjectPreviewLayout.tsx";
import {
	CommitDetails,
	CommitLabel,
	CommitsList,
	FileButton,
	FileDiff,
	formatHunkHeader,
	HunkDiff,
	ShowBranch,
	ShowCommitWithQuery,
} from "#ui/routes/project/$id/-shared.tsx";
import uiStyles from "#ui/ui.module.css";
import sharedStyles from "../-shared.module.css";
import { getDefaultSelection, normalizeBranchSelection, Selection } from "./-Selection.ts";

const isValidCommit = (commitId: string, branchDetails: BranchDetails): boolean => {
	const commitIds = new Set(branchDetails.commits.map((commit) => commit.id));
	return commitIds.has(commitId);
};

const getBranchRemote = (branch: BranchListing) => {
	if (branch.hasLocal) return null;
	return branch.remotes[0] ?? null;
};

const getBranchRef = (branch: BranchListing): string => {
	const remote = getBranchRemote(branch);
	if (remote === null) return `refs/heads/${branch.name}`;
	return `refs/remotes/${remote}/${branch.name}`;
};

const getExpandedCommitSelection = async ({
	branchName,
	commitId,
	projectId,
	queryClient,
}: {
	branchName: string;
	commitId: string;
	projectId: string;
	queryClient: ReturnType<typeof useQueryClient>;
}): Promise<Selection> => {
	const commitDetails = await queryClient.ensureQueryData(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	return {
		_tag: "Commit",
		branchName,
		commitId,
		mode: { _tag: "Details", path: commitDetails.changes[0]?.path },
	};
};

const BranchMenuPopup: FC<{
	branch: BranchListing;
	projectId: string;
	parts: typeof Menu | typeof ContextMenu;
}> = ({ branch, projectId, parts }) => {
	const { Popup, Item } = parts;
	const applyBranch = useMutation(applyBranchMutationOptions);
	const unapplyStack = useMutation(unapplyStackMutationOptions);
	const stackId = branch.stack?.id;

	return (
		<Popup className={classes(uiStyles.popup, uiStyles.menuPopup)}>
			{!branch.stack?.inWorkspace ? (
				<Item
					className={uiStyles.menuItem}
					onClick={() => {
						applyBranch.mutate({
							projectId,
							existingBranch: getBranchRef(branch),
						});
					}}
				>
					Apply branch
				</Item>
			) : (
				<Item
					className={uiStyles.menuItem}
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
	const stackId = branch.stack?.id;
	const isApplied = branch.stack?.inWorkspace ?? false;

	return isApplied ? (
		<button
			type="button"
			className={classes(sharedStyles.itemAction, styles.branchApplyToggle)}
			disabled={stackId === undefined}
			aria-label={`Unapply branch ${branch.name}`}
			onClick={() => {
				if (stackId === undefined) return;
				unapplyStack.mutate({ projectId, stackId });
			}}
		>
			<CheckIcon className={styles.branchApplyToggleDefaultIcon} />
			<ArrowUpIcon className={styles.branchApplyToggleHoverIcon} />
		</button>
	) : (
		<button
			type="button"
			className={classes(sharedStyles.itemAction, styles.branchApplyToggle)}
			aria-label={`Apply branch ${branch.name}`}
			onClick={() => {
				applyBranch.mutate({
					projectId,
					existingBranch: getBranchRef(branch),
				});
			}}
		>
			<AddCircleIcon className={styles.branchApplyToggleDefaultIcon} />
			<ArrowDownIcon className={styles.branchApplyToggleHoverIcon} />
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
	const branchSelection =
		selection?._tag === "Branch" && selection.branchName === branch.name ? selection : null;
	const commitSelection =
		selection?._tag === "Commit" && selection.branchName === branch.name ? selection : null;

	return (
		<div
			{...restProps}
			className={classes(
				sharedStyles.item,
				(branchSelection || commitSelection) && sharedStyles.selected,
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
								select({
									_tag: "Branch",
									branchName: branch.name,
								});
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
				<Menu.Trigger className={sharedStyles.itemAction} aria-label={`Branch ${branch.name} menu`}>
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
	const [isDetailsPending, startDetailsTransition] = useTransition();
	const queryClient = useQueryClient();
	const commitSelection =
		selection?._tag === "Commit" &&
		selection.branchName === branchName &&
		selection.commitId === commit.id
			? selection
			: null;

	const toggleDetails = () => {
		startDetailsTransition(async () => {
			if (commitSelection?.mode._tag === "Details") {
				select({
					_tag: "Commit",
					branchName,
					commitId: commit.id,
					mode: { _tag: "Summary" },
				});
				return;
			}

			select(
				await getExpandedCommitSelection({
					branchName,
					commitId: commit.id,
					projectId,
					queryClient,
				}),
			);
		});
	};

	return (
		<div
			className={classes(
				sharedStyles.item,
				commitSelection && sharedStyles.selected,
				isHighlighted && sharedStyles.highlighted,
			)}
			style={{ ...(isDetailsPending && { opacity: 0.5 }) }}
			aria-busy={isDetailsPending}
		>
			<button
				type="button"
				className={sharedStyles.commitButton}
				onClick={() => {
					select({
						_tag: "Commit",
						branchName,
						commitId: commit.id,
						mode: { _tag: "Summary" },
					});
				}}
			>
				<CommitLabel commit={commit} />
			</button>
			<button
				className={sharedStyles.itemAction}
				type="button"
				onClick={toggleDetails}
				aria-expanded={commitSelection?.mode._tag === "Details"}
				aria-label={
					commitSelection?.mode._tag === "Details" ? "Hide commit details" : "Show commit details"
				}
			>
				<ExpandCollapseIcon isExpanded={commitSelection?.mode._tag === "Details"} />
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
}> = ({ branchName, commit, projectId, selection, select }) => {
	const commitSelection =
		selection?._tag === "Commit" &&
		selection.branchName === branchName &&
		selection.commitId === commit.id
			? selection
			: null;

	return (
		<div>
			<CommitRow
				branchName={branchName}
				commit={commit}
				projectId={projectId}
				selection={selection}
				select={select}
				isHighlighted={false}
			/>
			{commitSelection?.mode._tag === "Details" && (
				<Suspense fallback={<div className={sharedStyles.itemEmpty}>Loading change details…</div>}>
					<CommitDetails
						projectId={projectId}
						commitId={commit.id}
						renderFile={(change) => (
							<div
								className={classes(
									sharedStyles.item,
									sharedStyles.file,
									commitSelection.mode._tag === "Details" &&
										commitSelection.mode.path === change.path &&
										sharedStyles.selectedFile,
								)}
							>
								<FileButton
									change={change}
									onClick={() => {
										select({
											_tag: "Commit",
											branchName,
											commitId: commit.id,
											mode: {
												_tag: "Details",
												path: change.path,
											},
										});
									}}
								/>
							</div>
						)}
					/>
				</Suspense>
			)}
		</div>
	);
};

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
	change: TreeChange;
	hunk: DiffHunk;
}> = ({ change, hunk }) => (
	<div>
		<div className={sharedStyles.hunkHeaderRow}>{formatHunkHeader(hunk)}</div>
		<HunkDiff change={change} diff={hunk.diff} />
	</div>
);

const ShowBranchCommitFile: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	commitId: string;
	path: string;
}> = ({ projectId, branchName, remote, commitId, path }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	if (!isValidCommit(commitId, branchDetails)) return null;

	const change = commitDetails.changes.find((candidate) => candidate.path === path);

	if (!change) return null;

	return (
		<FileDiff
			projectId={projectId}
			change={change}
			renderHunk={(hunk) => <Hunk change={change} hunk={hunk} />}
		/>
	);
};

const ShowBranchCommit: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	commitId: string;
}> = ({ projectId, branchName, remote, commitId }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);
	if (!isValidCommit(commitId, branchDetails)) return null;

	return (
		<ShowCommitWithQuery
			projectId={projectId}
			commitId={commitId}
			editable={false}
			renderHunk={(change, hunk) => <Hunk change={change} hunk={hunk} />}
		/>
	);
};

const Preview: FC<{
	projectId: string;
	selection: Selection;
	remote: string | null;
}> = ({ projectId, selection, remote }) =>
	Match.value(selection).pipe(
		Match.tag("Branch", ({ branchName }) => (
			<ShowBranch
				projectId={projectId}
				branchName={branchName}
				remote={remote}
				renderHunk={(change, hunk) => <Hunk change={change} hunk={hunk} />}
			/>
		)),
		Match.tag("Commit", ({ branchName, commitId, mode }) =>
			mode._tag === "Details" && mode.path !== undefined ? (
				<ShowBranchCommitFile
					projectId={projectId}
					branchName={branchName}
					remote={remote}
					commitId={commitId}
					path={mode.path}
				/>
			) : (
				<ShowBranchCommit
					projectId={projectId}
					branchName={branchName}
					remote={remote}
					commitId={commitId}
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
	const [_selection, select] = useState<Selection | null>(null);
	const selection =
		(_selection ? normalizeBranchSelection(_selection, sortedBranches) : null) ??
		getDefaultSelection(sortedBranches);
	const selectedBranch = sortedBranches.find((branch) => branch.name === selection?.branchName);

	if (!project) return <p>Project not found.</p>;

	return sortedBranches.length === 0 ? (
		<p>No branches found.</p>
	) : (
		<ProjectPreviewLayout
			projectId={projectId}
			preview={
				selection &&
				selectedBranch && (
					<Suspense fallback={<div>Loading preview…</div>}>
						<Preview
							projectId={projectId}
							selection={selection}
							remote={getBranchRemote(selectedBranch)}
						/>
					</Suspense>
				)
			}
		>
			<div className={sharedStyles.lanes}>
				<ul
					className={classes(
						styles.branchesListLane,
						selection?._tag === "Branch" ? sharedStyles.sectionSelected : undefined,
					)}
				>
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

				{selectedBranch && (
					<div
						className={classes(
							styles.branchDetailsLane,
							selection?._tag === "Commit" ? sharedStyles.sectionSelected : undefined,
						)}
					>
						<Suspense fallback={<div>Loading branch details…</div>}>
							<BranchDetailsC
								branchName={selectedBranch.name}
								projectId={projectId}
								remote={getBranchRemote(selectedBranch)}
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
