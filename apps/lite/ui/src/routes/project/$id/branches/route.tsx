import { applyBranchMutationOptions, unapplyStackMutationOptions } from "#ui/api/mutations.ts";
import {
	branchDetailsQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import { CheckIcon, ExpandCollapseIcon, MenuTriggerIcon } from "#ui/components/icons.tsx";
import { getShortcutAction } from "#ui/shortcuts.ts";
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
	isTypingTarget,
} from "#ui/routes/project/$id/-shared.tsx";
import { PositionedShortcutsBar } from "#ui/routes/project/$id/-ShortcutsBar.tsx";
import { ContextMenu, Menu } from "@base-ui/react";
import { BranchDetails, BranchListing, Commit, DiffHunk, TreeChange } from "@gitbutler/but-sdk";
import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Match } from "effect";
import { ComponentProps, FC, Suspense, useEffect, useEffectEvent, useTransition } from "react";
import useLocalStorageState from "use-local-storage-state";
import sharedStyles from "../-shared.module.css";
import {
	branchDetailsSelectionBindings,
	branchSummarySelectionBindings,
	commitDetailsSelectionBindings,
	commitSummarySelectionBindings,
	getAdjacentBranchSelection,
	getAdjacentCommitSelection,
	getDefaultSelection,
	getParentSelection,
	getSelectedBranchCommitId,
	getShortcutsBarMode,
	normalizeBranchSelection,
	type SharedSelectionAction,
} from "./-Selection.ts";
import {
	branchDetailsItem,
	BranchItem,
	branchSummaryItem,
	commitDetailsItem,
	type CommitItem,
	commitSummaryItem,
	type Item,
} from "./-Item.ts";
import styles from "./route.module.css";

const isValidCommit = (commitId: string, branchDetails: BranchDetails): boolean => {
	const commitIds = new Set(branchDetails.commits.map((commit) => commit.id));
	return commitIds.has(commitId);
};

const getBranchRemote = (branch: BranchListing) => {
	if (branch.hasLocal) return null;
	return branch.remotes[0] ?? null;
};

const getBranchRef = (branch: BranchListing): string | null => {
	if (branch.hasLocal) return `refs/heads/${branch.name}`;
	const remote = branch.remotes[0];
	if (remote === undefined) return null;
	return `refs/remotes/${remote}/${branch.name}`;
};

const getAdjacentCommitDetailsPath = ({
	paths,
	currentPath,
	offset,
}: {
	paths: Array<string>;
	currentPath: string | undefined;
	offset: -1 | 1;
}): string | null => {
	if (paths.length === 0) return null;
	if (currentPath === undefined) return offset > 0 ? (paths[0] ?? null) : (paths.at(-1) ?? null);

	const currentIndex = paths.indexOf(currentPath);
	if (currentIndex === -1) return offset > 0 ? (paths[0] ?? null) : (paths.at(-1) ?? null);
	return paths[currentIndex + offset] ?? null;
};

const getSelectedCommitPath = ({
	paths,
	selection,
}: {
	paths: Array<string>;
	selection: CommitItem;
}): string | undefined =>
	selection.mode._tag === "Details" &&
	selection.mode.path !== undefined &&
	paths.includes(selection.mode.path)
		? selection.mode.path
		: paths[0];

const getBranchByName = ({
	branches,
	branchName,
}: {
	branches: Array<BranchListing>;
	branchName: string;
}): BranchListing | null => branches.find((branch) => branch.name === branchName) ?? null;

const getBranchCommitIds = ({
	branches,
	branchName,
	projectId,
	queryClient,
}: {
	branches: Array<BranchListing>;
	branchName: string;
	projectId: string;
	queryClient: ReturnType<typeof useQueryClient>;
}): Array<string> | null => {
	const branch = getBranchByName({ branches, branchName });
	if (!branch) return null;

	const branchDetails = queryClient.getQueryData(
		branchDetailsQueryOptions({
			projectId,
			branchName,
			remote: getBranchRemote(branch),
		}).queryKey,
	);
	if (!branchDetails) return null;

	return branchDetails.commits.map((commit) => commit.id);
};

const useSelectionKeyboardShortcuts = ({
	branches,
	projectId,
	selection,
	select,
}: {
	branches: Array<BranchListing>;
	projectId: string;
	selection: Item | null;
	select: (selection: Item | null) => void;
}) => {
	const queryClient = useQueryClient();

	const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
		if (event.defaultPrevented) return;
		if (event.metaKey || event.ctrlKey || event.altKey) return;
		if (isTypingTarget(event.target)) return;
		if (!selection) return;

		const handleSharedAction = (action: SharedSelectionAction) => {
			const adjacentBranch = (offset: -1 | 1) =>
				select(getAdjacentBranchSelection(branches, selection, offset));

			Match.value(action).pipe(
				Match.tag("NextBranch", () => {
					adjacentBranch(1);
				}),
				Match.tag("PreviousBranch", () => {
					select(
						getParentSelection(selection) ?? getAdjacentBranchSelection(branches, selection, -1),
					);
				}),
				Match.exhaustive,
			);
		};

		Match.value(selection).pipe(
			Match.tag("Branch", (selection) => {
				Match.value(selection.mode).pipe(
					Match.tag("Summary", () => {
						const action = getShortcutAction(branchSummarySelectionBindings, selection, event);
						if (!action) return;

						event.preventDefault();

						Match.value(action).pipe(
							Match.tagsExhaustive({
								ExpandBranch: () => {
									select(branchDetailsItem(selection.branchName));
								},
								Move: ({ offset }) => {
									select(getAdjacentBranchSelection(branches, selection, offset));
								},
								NextBranch: () => handleSharedAction({ _tag: "NextBranch" }),
								PreviousBranch: () => handleSharedAction({ _tag: "PreviousBranch" }),
							}),
						);
					}),
					Match.tag("Details", () => {
						const action = getShortcutAction(branchDetailsSelectionBindings, selection, event);
						if (!action) return;

						event.preventDefault();

						Match.value(action).pipe(
							Match.tagsExhaustive({
								CloseBranch: () => {
									select(branchSummaryItem(selection.branchName));
								},
								Move: ({ offset }) => {
									const commitIds = getBranchCommitIds({
										branches,
										branchName: selection.branchName,
										projectId,
										queryClient,
									});
									if (!commitIds) return;
									const nextSelection = getAdjacentCommitSelection({
										branchName: selection.branchName,
										commitIds,
										selection,
										offset,
									});
									if (nextSelection) select(nextSelection);
								},
								NextBranch: () => handleSharedAction({ _tag: "NextBranch" }),
								PreviousBranch: () => handleSharedAction({ _tag: "PreviousBranch" }),
							}),
						);
					}),
					Match.exhaustive,
				);
			}),
			Match.tag("Commit", (selection) => {
				Match.value(selection.mode).pipe(
					Match.tag("Summary", () => {
						const action = getShortcutAction(commitSummarySelectionBindings, selection, event);
						if (!action) return;

						event.preventDefault();

						Match.value(action).pipe(
							Match.tagsExhaustive({
								ExpandCommit: () => {
									select(commitDetailsItem(selection.branchName, selection.commitId));
								},
								CloseBranch: () => {
									select(branchSummaryItem(selection.branchName));
								},
								Move: ({ offset }) => {
									const commitIds = getBranchCommitIds({
										branches,
										branchName: selection.branchName,
										projectId,
										queryClient,
									});
									if (!commitIds) return;
									const nextSelection = getAdjacentCommitSelection({
										branchName: selection.branchName,
										commitIds,
										selection,
										offset,
									});
									if (nextSelection) select(nextSelection);
								},
								NextBranch: () => handleSharedAction({ _tag: "NextBranch" }),
								PreviousBranch: () => handleSharedAction({ _tag: "PreviousBranch" }),
							}),
						);
					}),
					Match.tag("Details", () => {
						const action = getShortcutAction(commitDetailsSelectionBindings, selection, event);
						if (!action) return;

						event.preventDefault();

						Match.value(action).pipe(
							Match.tagsExhaustive({
								CloseCommitDetails: () => {
									select(commitSummaryItem(selection.branchName, selection.commitId));
								},
								Move: ({ offset }) => {
									const commitDetails = queryClient.getQueryData(
										commitDetailsWithLineStatsQueryOptions({
											projectId,
											commitId: selection.commitId,
										}).queryKey,
									);
									if (!commitDetails) return;

									const paths = commitDetails.changes.map((change) => change.path);
									const nextPath = getAdjacentCommitDetailsPath({
										paths,
										currentPath: getSelectedCommitPath({ paths, selection }),
										offset,
									});
									if (nextPath === null) return;
									select(commitDetailsItem(selection.branchName, selection.commitId, nextPath));
								},
								NextBranch: () => handleSharedAction({ _tag: "NextBranch" }),
								PreviousBranch: () => handleSharedAction({ _tag: "PreviousBranch" }),
							}),
						);
					}),
					Match.exhaustive,
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
			className={sharedStyles.itemAction}
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
			className={classes(sharedStyles.itemAction, styles.branchApplyButtonInactive)}
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
		selection: Item | null;
		select: (selection: Item | null) => void;
	} & ComponentProps<"div">
> = ({ projectId, branch, selection, select, className, ...restProps }) => {
	const branchSelection =
		selection?._tag === "Branch" && selection.branchName === branch.name ? selection : null;
	const commitSelection =
		selection?._tag === "Commit" && selection.branchName === branch.name ? selection : null;
	const isExpanded = branchSelection?.mode._tag === "Details" || commitSelection !== null;

	return (
		<div
			{...restProps}
			className={classes(
				sharedStyles.item,
				branchSelection || commitSelection ? sharedStyles.selected : undefined,
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
								select(branchSummaryItem(branch.name));
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

			<button
				className={sharedStyles.itemAction}
				type="button"
				onClick={() => {
					select(isExpanded ? branchSummaryItem(branch.name) : branchDetailsItem(branch.name));
				}}
				aria-expanded={isExpanded}
				aria-label={isExpanded ? "Hide branch commits" : "Show branch commits"}
			>
				<ExpandCollapseIcon isExpanded={isExpanded} />
			</button>
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
	selection: Item | null;
	select: (selection: Item | null) => void;
	isHighlighted: boolean;
}> = ({ branchName, commit, selection, select, isHighlighted }) => {
	const [isDetailsPending, startDetailsTransition] = useTransition();
	const commitSelection =
		selection?._tag === "Commit" &&
		selection.branchName === branchName &&
		selection.commitId === commit.id
			? selection
			: null;

	const toggleDetails = () => {
		startDetailsTransition(() => {
			if (commitSelection?.mode._tag === "Details") {
				select(commitSummaryItem(branchName, commit.id));
				return;
			}

			select(commitDetailsItem(branchName, commit.id));
		});
	};

	return (
		<div
			className={classes(
				sharedStyles.item,
				commitSelection ? sharedStyles.selected : undefined,
				isHighlighted && sharedStyles.highlighted,
			)}
			style={{ ...(isDetailsPending && { opacity: 0.5 }) }}
			aria-busy={isDetailsPending}
		>
			<button
				type="button"
				className={sharedStyles.commitButton}
				onClick={() => {
					select(commitSummaryItem(branchName, commit.id));
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

const BranchCommitDetails: FC<{
	branchName: string;
	commitId: string;
	commitSelection: CommitItem;
	projectId: string;
	select: (selection: Item | null) => void;
}> = ({ branchName, commitId, commitSelection, projectId, select }) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({
			projectId,
			commitId,
		}),
	);
	const paths = commitDetails.changes.map((change: TreeChange) => change.path);
	const selectedPath = getSelectedCommitPath({
		paths,
		selection: commitSelection,
	});

	return (
		<CommitDetails
			projectId={projectId}
			commitId={commitId}
			renderFile={(change) => (
				<div
					className={classes(
						sharedStyles.item,
						selectedPath === change.path && sharedStyles.selectedFile,
					)}
				>
					<FileButton
						change={change}
						onClick={() => {
							select(commitDetailsItem(branchName, commitId, change.path));
						}}
					/>
				</div>
			)}
		/>
	);
};

const CommitC: FC<{
	branchName: string;
	commit: Commit;
	isSelected: boolean;
	projectId: string;
	selection: Item | null;
	select: (selection: Item | null) => void;
}> = ({ branchName, commit, isSelected, projectId, selection, select }) => {
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
				isHighlighted={false}
				selection={
					isSelected && commitSelection === null
						? commitSummaryItem(branchName, commit.id)
						: selection
				}
				select={select}
			/>
			{commitSelection?.mode._tag === "Details" && (
				<div className={sharedStyles.commitDetails}>
					<Suspense
						fallback={<div className={sharedStyles.itemEmpty}>Loading change details…</div>}
					>
						<BranchCommitDetails
							branchName={branchName}
							commitSelection={commitSelection}
							projectId={projectId}
							commitId={commit.id}
							select={select}
						/>
					</Suspense>
				</div>
			)}
		</div>
	);
};

const BranchCommitsC: FC<{
	branch: BranchListing;
	projectId: string;
	selection: Item | null;
	select: (selection: Item | null) => void;
}> = ({ branch, projectId, selection, select }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({
			projectId,
			branchName: branch.name,
			remote: getBranchRemote(branch),
		}),
	);
	const selectedCommitId =
		selection &&
		((selection._tag === "Branch" && selection.branchName === branch.name) ||
			(selection._tag === "Commit" && selection.branchName === branch.name))
			? getSelectedBranchCommitId({
					commitIds: branchDetails.commits.map((commit) => commit.id),
					selection,
				})
			: undefined;

	return branchDetails.commits.length === 0 ? (
		<div className={sharedStyles.itemEmpty}>No commits.</div>
	) : (
		<CommitsList commits={branchDetails.commits}>
			{(commit) => (
				<CommitC
					branchName={branch.name}
					commit={commit}
					isSelected={selectedCommitId === commit.id}
					projectId={projectId}
					selection={selection}
					select={select}
				/>
			)}
		</CommitsList>
	);
};

const BranchC: FC<{
	branch: BranchListing;
	projectId: string;
	selection: Item | null;
	select: (selection: Item | null) => void;
}> = ({ branch, projectId, selection, select }) => {
	const isExpanded =
		(selection?._tag === "Branch" &&
			selection.branchName === branch.name &&
			selection.mode._tag === "Details") ||
		(selection?._tag === "Commit" && selection.branchName === branch.name);
	const isSectionSelected =
		(selection?._tag === "Branch" && selection.branchName === branch.name) ||
		(selection?._tag === "Commit" && selection.branchName === branch.name);

	return (
		<div className={classes(isSectionSelected && sharedStyles.sectionSelected)}>
			<BranchRow projectId={projectId} branch={branch} selection={selection} select={select} />
			{isExpanded && (
				<div className={sharedStyles.commitDetails}>
					<Suspense
						fallback={<div className={sharedStyles.itemEmpty}>Loading branch details…</div>}
					>
						<BranchCommitsC
							branch={branch}
							projectId={projectId}
							selection={selection}
							select={select}
						/>
					</Suspense>
				</div>
			)}
		</div>
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
			renderHunk={(change, hunk) => <Hunk change={change} hunk={hunk} />}
		/>
	);
};

const ShowBranchCommitOrBranch: FC<{
	projectId: string;
	selection: BranchItem;
	remote: string | null;
	branchRef: string | null;
}> = ({ projectId, selection, remote, branchRef }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName: selection.branchName, remote }),
	);
	const selectedCommitId =
		selection.mode._tag === "Details"
			? getSelectedBranchCommitId({
					commitIds: branchDetails.commits.map((commit) => commit.id),
					selection: { _tag: "Branch", ...selection },
				})
			: undefined;

	return selectedCommitId !== undefined ? (
		<ShowBranchCommit
			projectId={projectId}
			branchName={selection.branchName}
			remote={remote}
			commitId={selectedCommitId}
		/>
	) : branchRef !== null ? (
		<ShowBranch
			projectId={projectId}
			branchRef={branchRef}
			branchName={selection.branchName}
			remote={remote}
			renderHunk={(change, hunk) => <Hunk change={change} hunk={hunk} />}
		/>
	) : (
		<div>No branch diff available.</div>
	);
};

const ShowBranchCommitOrFile: FC<{
	projectId: string;
	selection: CommitItem;
	remote: string | null;
}> = ({ projectId, selection, remote }) => {
	const { commitId, branchName } = selection;
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	const paths = commitDetails.changes.map((change) => change.path);
	const selectedPath =
		selection.mode._tag === "Details" ? getSelectedCommitPath({ paths, selection }) : undefined;

	return selectedPath !== undefined ? (
		<ShowBranchCommitFile
			projectId={projectId}
			branchName={branchName}
			remote={remote}
			commitId={commitId}
			path={selectedPath}
		/>
	) : (
		<ShowBranchCommit
			projectId={projectId}
			branchName={branchName}
			remote={remote}
			commitId={commitId}
		/>
	);
};

const Preview: FC<{
	projectId: string;
	selection: Item;
	remote: string | null;
	branchRef: string | null;
}> = ({ projectId, selection, remote, branchRef }) =>
	Match.value(selection).pipe(
		Match.tag("Branch", (selection) => (
			<ShowBranchCommitOrBranch
				projectId={projectId}
				selection={selection}
				remote={remote}
				branchRef={branchRef}
			/>
		)),
		Match.tag("Commit", (selection) => (
			<ShowBranchCommitOrFile projectId={projectId} selection={selection} remote={remote} />
		)),
		Match.exhaustive,
	);

const ProjectBranchesPage: FC = () => {
	const { id: projectId } = Route.useParams();

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());
	const project = projects.find((project) => project.id === projectId);
	const { data: branches } = useSuspenseQuery(listBranchesQueryOptions(projectId));

	const sortedBranches = branches.slice().sort((a, b) => a.name.localeCompare(b.name));
	const [_selection, select] = useLocalStorageState<Item | null>(
		`project:${projectId}:branches:selection`,
		{ defaultValue: null },
	);
	const selection =
		(_selection ? normalizeBranchSelection(_selection, sortedBranches) : null) ??
		getDefaultSelection(sortedBranches);
	const selectedBranch = sortedBranches.find((branch) => branch.name === selection?.branchName);

	useSelectionKeyboardShortcuts({ branches: sortedBranches, projectId, selection, select });

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
							branchRef={getBranchRef(selectedBranch)}
							remote={getBranchRemote(selectedBranch)}
						/>
					</Suspense>
				)
			}
		>
			<div className={sharedStyles.lanes}>
				<ul className={styles.branchTreeLane}>
					{sortedBranches.map((branch) => (
						<li key={branch.name}>
							<BranchC
								branch={branch}
								projectId={projectId}
								selection={selection}
								select={select}
							/>
						</li>
					))}
				</ul>
			</div>

			<PositionedShortcutsBar mode={getShortcutsBarMode({ selection })} />
		</ProjectPreviewLayout>
	);
};

export const Route = createFileRoute("/project/$id/branches")({
	component: ProjectBranchesPage,
});
