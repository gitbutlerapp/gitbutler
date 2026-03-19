import useLocalStorageState from "use-local-storage-state";
import { ContextMenu, Menu } from "@base-ui/react";
import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { createRoute } from "@tanstack/react-router";
import { ComponentProps, FC, Suspense } from "react";
import {
	CommitDetails,
	CommitRow,
	CommitsList,
	FileDiff,
	FileButton,
	Hunk,
	classes,
} from "#ui/routes/project-shared.tsx";
import { ProjectPanelLayout } from "#ui/routes/ProjectPanelLayout.tsx";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { BranchDetails, BranchIdentity, BranchListing, Commit } from "@gitbutler/but-sdk";
import styles from "./project-branches.module.css";
import sharedStyles from "./project-shared.module.css";
import { applyBranchMutationOptions, unapplyStackMutationOptions } from "#ui/mutations.ts";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/queries.ts";
import { Match } from "effect";

type Selection =
	| {
			_tag: "Branch";
			branchName: BranchIdentity;
	  }
	| {
			_tag: "Commit";
			branchName: BranchIdentity;
			commitId: string;
	  }
	| {
			_tag: "CommitFile";
			branchName: BranchIdentity;
			commitId: string;
			path: string;
	  };

const normalizeBranchSelection = (
	selection: Selection,
	branches: Array<BranchListing>,
): Selection | null => {
	const branch = branches.find((branch) => branch.name === selection.branchName);
	if (!branch) return null;
	return selection;
};

const getDefaultSelection = (branches: Array<BranchListing>): Selection | null => {
	const firstBranch = branches[0];
	if (!firstBranch) return null;
	return { _tag: "Branch", branchName: firstBranch.name };
};

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

const BranchRow: FC<
	{
		projectId: string;
		branch: BranchListing;
		isSelected: boolean;
		isSelectedWithin: boolean;
		toggleSelect: () => void;
	} & ComponentProps<"div">
> = ({
	projectId,
	branch,
	isSelected,
	isSelectedWithin,
	toggleSelect,
	className,
	...restProps
}) => (
	<div
		{...restProps}
		className={classes(
			styles.branchRow,
			isSelected
				? sharedStyles.selected
				: isSelectedWithin
					? sharedStyles.selectedWithin
					: undefined,
			className,
		)}
	>
		<ContextMenu.Root>
			<ContextMenu.Trigger
				render={
					<button type="button" className={styles.branchButton} onClick={toggleSelect}>
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
		<Menu.Root>
			<Menu.Trigger style={{ lineHeight: 1 }} className={sharedStyles.commitMenuTrigger}>
				𑁔
			</Menu.Trigger>
			<Menu.Portal>
				<Menu.Positioner align="end">
					<BranchMenuPopup branch={branch} projectId={projectId} parts={Menu} />
				</Menu.Positioner>
			</Menu.Portal>
		</Menu.Root>
	</div>
);

const CommitC: FC<{
	projectId: string;
	commit: Commit;
	isSelected: boolean;
	isAnyFileSelected: boolean;
	isFileSelected: (path: string) => boolean;
	toggleExpand: () => Promise<void> | void;
	toggleSelect: () => void;
	toggleFileSelect: (path: string) => void;
}> = ({
	projectId,
	commit,
	isSelected,
	isAnyFileSelected,
	isFileSelected,
	toggleExpand,
	toggleSelect,
	toggleFileSelect,
}) => (
	<div className={sharedStyles.commit}>
		<CommitRow
			projectId={projectId}
			commit={commit}
			isSelected={isSelected}
			isAnyFileSelected={isAnyFileSelected}
			isHighlighted={false}
			toggleExpand={toggleExpand}
			toggleSelect={toggleSelect}
		/>
		{isAnyFileSelected && (
			<div className={sharedStyles.commitDetails}>
				<Suspense fallback={<div>Loading changed details…</div>}>
					<CommitDetails
						projectId={projectId}
						commitId={commit.id}
						renderFile={(change) => (
							<div
								className={classes(
									sharedStyles.fileRow,
									isFileSelected(change.path) && sharedStyles.selected,
								)}
							>
								<FileButton change={change} toggleSelect={() => toggleFileSelect(change.path)} />
							</div>
						)}
					/>
				</Suspense>
			</div>
		)}
	</div>
);

const BranchDetailsC: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	isCommitSelected: (commitId: string) => boolean;
	isCommitAnyFileSelected: (commitId: string) => boolean;
	isCommitFileSelected: (commitId: string, path: string) => boolean;
	toggleCommitExpanded: (commitId: string) => Promise<void> | void;
	toggleCommitSelection: (commitId: string) => void;
	toggleCommitFileSelection: (commitId: string, path: string) => void;
}> = ({
	projectId,
	branchName,
	remote,
	isCommitSelected,
	isCommitAnyFileSelected,
	isCommitFileSelected,
	toggleCommitExpanded,
	toggleCommitSelection,
	toggleCommitFileSelection,
}) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);

	return (
		<div>
			<h3>{branchDetails.name} commits</h3>
			<CommitsList commits={branchDetails.commits}>
				{(commit) => (
					<CommitC
						projectId={projectId}
						commit={commit}
						isSelected={isCommitSelected(commit.id)}
						isAnyFileSelected={isCommitAnyFileSelected(commit.id)}
						isFileSelected={(path) => isCommitFileSelected(commit.id, path)}
						toggleExpand={() => toggleCommitExpanded(commit.id)}
						toggleSelect={() => {
							toggleCommitSelection(commit.id);
						}}
						toggleFileSelect={(path) => {
							toggleCommitFileSelection(commit.id, path);
						}}
					/>
				)}
			</CommitsList>
		</div>
	);
};

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
		<FileDiff
			projectId={projectId}
			change={change}
			renderHunk={(hunk, patch) => (
				<Hunk patch={patch} changeUnit={{ _tag: "Commit", commitId }} change={change} hunk={hunk} />
			)}
		/>
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

	if (data.changes.length === 0) return null;

	return (
		<ul>
			{data.changes.map((change) => (
				<li key={change.path}>
					<h5>{change.path}</h5>
					<FileDiff
						projectId={projectId}
						change={change}
						renderHunk={(hunk, patch) => (
							<Hunk
								patch={patch}
								changeUnit={{ _tag: "Commit", commitId }}
								change={change}
								hunk={hunk}
							/>
						)}
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
								renderHunk={(hunk, patch) => (
									<Hunk
										patch={patch}
										// TODO: this doesn't make sense
										changeUnit={{ _tag: "Changes", stackId: null }}
										change={change}
										hunk={hunk}
									/>
								)}
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
		Match.tag("Commit", ({ branchName, commitId }) => (
			<CommitDiff
				projectId={projectId}
				branchName={branchName}
				remote={remote}
				commitId={commitId}
			/>
		)),
		Match.tag("CommitFile", ({ branchName, commitId, path }) => (
			<CommitFileDiff
				projectId={projectId}
				branchName={branchName}
				remote={remote}
				commitId={commitId}
				path={path}
			/>
		)),
		Match.exhaustive,
	);

const ProjectBranchesPage: FC = () => {
	const { id: projectId } = projectBranchesRoute.useParams();

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());
	const project = projects.find((project) => project.id === projectId);
	const { data: branches } = useSuspenseQuery(listBranchesQueryOptions(projectId));
	const queryClient = useQueryClient();

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

	const isBranchSelected = (branchName: string) =>
		selection?._tag === "Branch" && selection.branchName === branchName;
	const isBranchSelectedWithin = (branchName: string) =>
		(selection?._tag === "Commit" || selection?._tag === "CommitFile") &&
		selection.branchName === branchName;

	const isCommitSelected = (branchName: string, commitId: string) =>
		selection?._tag === "Commit" &&
		selection.branchName === branchName &&
		selection.commitId === commitId;
	const isCommitAnyFileSelected = (branchName: string, commitId: string) =>
		selection?._tag === "CommitFile" &&
		selection.branchName === branchName &&
		selection.commitId === commitId;
	const isCommitFileSelected = (branchName: string, commitId: string, path: string) =>
		selection?._tag === "CommitFile" &&
		selection.branchName === branchName &&
		selection.commitId === commitId &&
		selection.path === path;

	const toggleCommitSelection = (branchName: string, commitId: string) => {
		select(
			isCommitSelected(branchName, commitId)
				? { _tag: "Branch", branchName }
				: { _tag: "Commit", branchName, commitId },
		);
	};
	const toggleCommitExpanded = async (branchName: string, commitId: string) => {
		if (isCommitAnyFileSelected(branchName, commitId)) {
			select({ _tag: "Commit", branchName, commitId });
			return;
		}

		const commitDetails = await queryClient.ensureQueryData(
			commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
		);
		const firstPath = commitDetails.changes[0]?.path;

		select(
			firstPath !== undefined
				? { _tag: "CommitFile", branchName, commitId, path: firstPath }
				: { _tag: "Commit", branchName, commitId },
		);
	};
	const toggleCommitFileSelection = (branchName: string, commitId: string, path: string) => {
		select(
			isCommitFileSelected(branchName, commitId, path)
				? { _tag: "Commit", branchName, commitId }
				: { _tag: "CommitFile", branchName, commitId, path },
		);
	};
	const toggleBranchSelection = (branchName: string) => {
		select((selected) =>
			selected?.branchName === branchName ? null : { _tag: "Branch", branchName },
		);
	};

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	return sortedBranches.length === 0 ? (
		<p>No branches found.</p>
	) : (
		<ProjectPanelLayout
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
				<ul className={styles.branchesList}>
					{sortedBranches.map((branch) => {
						const isSelected = isBranchSelected(branch.name);
						const isSelectedWithin = isBranchSelectedWithin(branch.name);
						return (
							<li key={branch.name}>
								<BranchRow
									projectId={projectId}
									branch={branch}
									isSelected={isSelected}
									isSelectedWithin={isSelectedWithin}
									toggleSelect={() => {
										toggleBranchSelection(branch.name);
									}}
								/>
							</li>
						);
					})}
				</ul>

				{selectedBranch?.name != null && (
					<div className={sharedStyles.commitsLane}>
						<Suspense fallback={<div>Loading branch details…</div>}>
							<BranchDetailsC
								projectId={projectId}
								branchName={selectedBranch.name}
								remote={selectedRemote ?? null}
								isCommitSelected={(commitId) => isCommitSelected(selectedBranch.name, commitId)}
								isCommitAnyFileSelected={(commitId) =>
									isCommitAnyFileSelected(selectedBranch.name, commitId)
								}
								isCommitFileSelected={(commitId, path) =>
									isCommitFileSelected(selectedBranch.name, commitId, path)
								}
								toggleCommitExpanded={(commitId) =>
									toggleCommitExpanded(selectedBranch.name, commitId)
								}
								toggleCommitSelection={(commitId) =>
									toggleCommitSelection(selectedBranch.name, commitId)
								}
								toggleCommitFileSelection={(commitId, path) =>
									toggleCommitFileSelection(selectedBranch.name, commitId, path)
								}
							/>
						</Suspense>
					</div>
				)}
			</div>
		</ProjectPanelLayout>
	);
};

export const projectBranchesRoute = createRoute({
	getParentRoute: () => projectRootRoute,
	path: "branches",
	component: ProjectBranchesPage,
});
