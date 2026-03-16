import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import { createRoute } from "@tanstack/react-router";
import { FC, Suspense } from "react";
import {
	CommitButton,
	CommitDetails,
	CommitsList,
	FileDiff,
	FileButton,
	HunkListItem,
	hunkKey,
	classes,
} from "#ui/routes/project-shared.tsx";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { BranchListing, Commit, TreeChange } from "@gitbutler/but-sdk";
import styles from "./project-branches.module.css";
import sharedStyles from "./project-shared.module.css";
import { useLocalStorageState } from "#ui/hooks/useLocalStorageState.ts";
import { applyBranchMutationOptions, unapplyStackMutationOptions } from "#ui/mutations.ts";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/queries.ts";

type Selection = {
	commitId: string;
	path?: string;
};

const getBranchRef = (branch: BranchListing): string | null => {
	if (branch.hasLocal) return `refs/heads/${branch.name}`;
	const remote = branch.remotes[0];
	if (remote === undefined) return null;
	return `refs/remotes/${remote}/${branch.name}`;
};

const File: FC<{
	change: TreeChange;
	isSelected: boolean;
	toggleSelect: () => void;
}> = ({ change, isSelected, toggleSelect }) => (
	<li>
		<div className={sharedStyles.fileRow}>
			<FileButton change={change} isSelected={isSelected} toggleSelect={toggleSelect} />
		</div>
	</li>
);

const CommitC: FC<{
	projectId: string;
	commit: Commit;
	isSelected: boolean;
	isAnyFileSelected: boolean;
	isFileSelected: (path: string) => boolean;
	toggleSelect: () => void;
	toggleFileSelect: (path: string) => void;
}> = ({
	projectId,
	commit,
	isSelected,
	isAnyFileSelected,
	isFileSelected,
	toggleSelect,
	toggleFileSelect,
}) => {
	const expanded = isSelected || isAnyFileSelected;

	return (
		<li className={sharedStyles.commitsListItem}>
			<CommitButton
				commit={commit}
				isSelected={isSelected}
				isAnyFileSelected={isAnyFileSelected}
				isHighlighted={false}
				toggleSelect={toggleSelect}
			/>
			{expanded && (
				<div className={sharedStyles.commitDetails}>
					<Suspense fallback={<div>Loading changed details…</div>}>
						<CommitDetails
							projectId={projectId}
							commitId={commit.id}
							renderFile={(change) => (
								<File
									key={change.path}
									isSelected={isFileSelected(change.path)}
									toggleSelect={() => toggleFileSelect(change.path)}
									change={change}
								/>
							)}
						/>
					</Suspense>
				</div>
			)}
		</li>
	);
};

const BranchDetails: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	selection: Selection | null;
	select: (selection: Selection | null) => void;
}> = ({ projectId, branchName, remote, selection, select }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);

	const isCommitSelected = (commitId: string) =>
		selection?.commitId === commitId && selection.path === undefined;

	const isCommitAnyFileSelected = (commitId: string) =>
		selection?.commitId === commitId && selection.path !== undefined;

	const isCommitFileSelected = (commitId: string, path: string) =>
		selection?.commitId === commitId && selection.path === path;

	const toggleCommitSelection = (commitId: string): Selection | null =>
		isCommitSelected(commitId) ? null : { commitId };

	const toggleCommitFileSelection = (commitId: string, path: string): Selection | null =>
		isCommitFileSelected(commitId, path) ? { commitId } : { commitId, path };

	return (
		<div>
			<h3>{branchDetails.name} commits</h3>
			<CommitsList commits={branchDetails.commits}>
				{(commit) => (
					<CommitC
						key={commit.id}
						projectId={projectId}
						commit={commit}
						isSelected={isCommitSelected(commit.id)}
						isAnyFileSelected={isCommitAnyFileSelected(commit.id)}
						isFileSelected={(path) => isCommitFileSelected(commit.id, path)}
						toggleSelect={() => {
							select(toggleCommitSelection(commit.id));
						}}
						toggleFileSelect={(path) => {
							select(toggleCommitFileSelection(commit.id, path));
						}}
					/>
				)}
			</CommitsList>
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
					<HunkListItem
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
								<HunkListItem
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

const SelectedLaneDiff: FC<{
	projectId: string;
	selection: Selection;
}> = ({ projectId, selection }) =>
	selection.path !== undefined ? (
		<SelectedCommitFileDiff
			projectId={projectId}
			commitId={selection.commitId}
			path={selection.path}
		/>
	) : (
		<SelectedCommitDiff projectId={projectId} commitId={selection.commitId} />
	);

const SelectedBranchDiff: FC<{
	projectId: string;
	branch: string;
}> = ({ projectId, branch }) => {
	const { data } = useSuspenseQuery(branchDiffQueryOptions({ projectId, branch }));

	if (data.changes.length === 0) return <div>No file changes.</div>;

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
								<HunkListItem
									key={hunkKey(hunk)}
									patch={patch}
									// TODO: this doesn't make sense
									changeUnit={{ _tag: "changes", stackId: null }}
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

const BranchDetailsLane: FC<{
	projectId: string;
	branchName: string;
	branchRef: string | null;
	remote: string | null;
}> = ({ projectId, branchName, branchRef, remote }) => {
	const [selection, select] = useLocalStorageState<Selection | null>(
		`branchSelection:${projectId}:${branchName}`,
		null,
	);

	return (
		<div className={sharedStyles.lane}>
			<div className={sharedStyles.laneMain}>
				<Suspense fallback={<div>Loading branch details…</div>}>
					<BranchDetails
						projectId={projectId}
						branchName={branchName}
						remote={remote}
						selection={selection}
						select={select}
					/>
				</Suspense>
			</div>

			<Suspense fallback={<div>Loading diff…</div>}>
				{selection !== null ? (
					<SelectedLaneDiff projectId={projectId} selection={selection} />
				) : branchRef !== null ? (
					<SelectedBranchDiff projectId={projectId} branch={branchRef} />
				) : (
					<div>No branch diff available.</div>
				)}
			</Suspense>
		</div>
	);
};

const ProjectBranchesPage: FC = () => {
	const { id } = projectBranchesRoute.useParams();
	const [selectedBranchName, setSelectedBranchName] = useLocalStorageState<string | null>(
		`selectedBranchName:${id}`,
		null,
	);

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());
	const project = projects.find((project) => project.id === id);
	const { data: branches } = useSuspenseQuery(listBranchesQueryOptions(id));
	const applyBranch = useMutation(applyBranchMutationOptions);
	const unapplyStack = useMutation(unapplyStackMutationOptions);

	const sortedBranches = branches.slice().sort((a, b) => a.name.localeCompare(b.name));
	const selectedBranch =
		sortedBranches.find((branch) => branch.name === selectedBranchName) ??
		sortedBranches[0] ??
		null;
	const selectedBranchResolvedName = selectedBranch?.name ?? null;

	const selectedRemote =
		selectedBranch && !selectedBranch.hasLocal ? (selectedBranch.remotes[0] ?? null) : null;

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	return (
		<>
			<h2>{project.title} branches</h2>
			{sortedBranches.length === 0 ? (
				<p>No branches found.</p>
			) : (
				<div className={styles.branchesLayout}>
					<ul className={styles.branchesList}>
						{sortedBranches.map((branch) => {
							const ref = getBranchRef(branch);
							const stackId = branch.stack?.id;
							const isSelected = selectedBranchResolvedName === branch.name;
							return (
								<li key={branch.name} className={styles.branchesListItem}>
									<button
										type="button"
										className={classes(styles.branchButton, isSelected && sharedStyles.selected)}
										onClick={() => {
											setSelectedBranchName(branch.name);
										}}
									>
										{branch.name}
										{branch.stack?.branches && branch.stack.branches.length > 1 && (
											<> (+{branch.stack.branches.length - 1} more)</>
										)}
									</button>
									{!branch.stack?.inWorkspace ? (
										<button
											type="button"
											disabled={applyBranch.isPending || ref === null}
											onClick={() => {
												if (ref === null) return;
												applyBranch.mutate({
													projectId: id,
													existingBranch: ref,
												});
											}}
										>
											{applyBranch.isPending ? "Applying branch…" : "Apply branch"}
										</button>
									) : (
										stackId != null && (
											<button
												type="button"
												disabled={unapplyStack.isPending}
												onClick={() => {
													unapplyStack.mutate({
														projectId: id,
														stackId,
													});
												}}
											>
												{unapplyStack.isPending ? "Unapplying stack…" : "Unapply stack"}
											</button>
										)
									)}
								</li>
							);
						})}
					</ul>

					{selectedBranchResolvedName !== null && (
						<div className={styles.branchesDetailsPane}>
							<BranchDetailsLane
								key={selectedBranchResolvedName}
								projectId={id}
								branchName={selectedBranchResolvedName}
								branchRef={selectedBranch ? getBranchRef(selectedBranch) : null}
								remote={selectedRemote}
							/>
						</div>
					)}
				</div>
			)}
		</>
	);
};

export const projectBranchesRoute = createRoute({
	getParentRoute: () => projectRootRoute,
	path: "branches",
	component: ProjectBranchesPage,
});
