import { useMutation, useSuspenseQuery } from "@tanstack/react-query";
import { createRoute } from "@tanstack/react-router";
import { FC, Suspense } from "react";
import {
	CommitDetails,
	CommitRow,
	CommitsList,
	FileDiff,
	FileButton,
	Hunk,
	classes,
} from "#ui/routes/project-shared.tsx";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { BranchListing, Commit } from "@gitbutler/but-sdk";
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
	branchName: string;
	commitId?: string;
	path?: string;
};

const getBranchRef = (branch: BranchListing): string | null => {
	if (branch.hasLocal) return `refs/heads/${branch.name}`;
	const remote = branch.remotes[0];
	if (remote === undefined) return null;
	return `refs/remotes/${remote}/${branch.name}`;
};

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
		<div className={sharedStyles.commit}>
			<CommitRow
				projectId={projectId}
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
								<div className={sharedStyles.fileRow}>
									<FileButton
										change={change}
										isSelected={isFileSelected(change.path)}
										toggleSelect={() => toggleFileSelect(change.path)}
									/>
								</div>
							)}
						/>
					</Suspense>
				</div>
			)}
		</div>
	);
};

const BranchDetails: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	isCommitSelected: (commitId: string) => boolean;
	isCommitAnyFileSelected: (commitId: string) => boolean;
	isCommitFileSelected: (commitId: string, path: string) => boolean;
	toggleCommitSelection: (commitId: string) => void;
	toggleCommitFileSelection: (commitId: string, path: string) => void;
}> = ({
	projectId,
	branchName,
	remote,
	isCommitSelected,
	isCommitAnyFileSelected,
	isCommitFileSelected,
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
		<FileDiff
			projectId={projectId}
			change={change}
			renderHunk={(hunk, patch) => (
				<Hunk patch={patch} changeUnit={{ _tag: "commit", commitId }} change={change} hunk={hunk} />
			)}
		/>
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
		<ul className={sharedStyles.hunks}>
			{data.changes.map((change) => (
				<li key={change.path}>
					<h5>{change.path}</h5>
					<FileDiff
						projectId={projectId}
						change={change}
						renderHunk={(hunk, patch) => (
							<Hunk
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
	);
};

const SelectedBranchDiff: FC<{
	projectId: string;
	branch: string;
}> = ({ projectId, branch }) => {
	const { data } = useSuspenseQuery(branchDiffQueryOptions({ projectId, branch }));

	if (data.changes.length === 0) return <div>No file changes.</div>;

	return (
		<ul className={sharedStyles.hunks}>
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
								changeUnit={{ _tag: "changes", stackId: null }}
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

const BranchDetailsLane: FC<{
	projectId: string;
	branchName: string;
	remote: string | null;
	isCommitSelected: (commitId: string) => boolean;
	isCommitAnyFileSelected: (commitId: string) => boolean;
	isCommitFileSelected: (commitId: string, path: string) => boolean;
	toggleCommitSelection: (commitId: string) => void;
	toggleCommitFileSelection: (commitId: string, path: string) => void;
}> = ({
	projectId,
	branchName,
	remote,
	isCommitSelected,
	isCommitAnyFileSelected,
	isCommitFileSelected,
	toggleCommitSelection,
	toggleCommitFileSelection,
}) => (
	<div className={sharedStyles.commitsLane}>
		<Suspense fallback={<div>Loading branch details…</div>}>
			<BranchDetails
				projectId={projectId}
				branchName={branchName}
				remote={remote}
				isCommitSelected={isCommitSelected}
				isCommitAnyFileSelected={isCommitAnyFileSelected}
				isCommitFileSelected={isCommitFileSelected}
				toggleCommitSelection={toggleCommitSelection}
				toggleCommitFileSelection={toggleCommitFileSelection}
			/>
		</Suspense>
	</div>
);

const ProjectBranchesPage: FC = () => {
	const { id } = projectBranchesRoute.useParams();

	const [selection, select] = useLocalStorageState<Selection | null>(
		`project:${id}:branches:selection`,
		null,
	);

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions());
	const project = projects.find((project) => project.id === id);
	const { data: branches } = useSuspenseQuery(listBranchesQueryOptions(id));
	const applyBranch = useMutation(applyBranchMutationOptions);
	const unapplyStack = useMutation(unapplyStackMutationOptions);

	const sortedBranches = branches.slice().sort((a, b) => a.name.localeCompare(b.name));
	const selectedBranch = sortedBranches.find((branch) => branch.name === selection?.branchName);
	const selectedBranchResolvedName = selectedBranch?.name;
	const selectedBranchRef = selectedBranch ? getBranchRef(selectedBranch) : null;

	const isCommitSelected = (branchName: string, commitId: string) =>
		selection?.branchName === branchName &&
		selection.commitId === commitId &&
		selection.path === undefined;
	const isCommitAnyFileSelected = (branchName: string, commitId: string) =>
		selection?.branchName === branchName &&
		selection.commitId === commitId &&
		selection.path !== undefined;
	const isCommitFileSelected = (branchName: string, commitId: string, path: string) =>
		selection?.branchName === branchName &&
		selection.commitId === commitId &&
		selection.path === path;
	const toggleCommitSelection = (branchName: string, commitId: string) => {
		select(isCommitSelected(branchName, commitId) ? { branchName } : { branchName, commitId });
	};
	const toggleCommitFileSelection = (branchName: string, commitId: string, path: string) => {
		select(
			isCommitFileSelected(branchName, commitId, path)
				? { branchName, commitId }
				: { branchName, commitId, path },
		);
	};

	const selectedRemote =
		selectedBranch && !selectedBranch.hasLocal ? selectedBranch.remotes[0] : null;

	// TODO: dedupe
	if (!project) return <p>Project not found.</p>;

	return sortedBranches.length === 0 ? (
		<p>No branches found.</p>
	) : (
		<>
			<div className={sharedStyles.lanes}>
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
										select((selected) =>
											selected?.branchName === branch.name ? null : { branchName: branch.name },
										);
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

				{selectedBranchResolvedName != null && (
					<BranchDetailsLane
						key={selectedBranchResolvedName}
						projectId={id}
						branchName={selectedBranchResolvedName}
						remote={selectedRemote ?? null}
						isCommitSelected={(commitId) => isCommitSelected(selectedBranchResolvedName, commitId)}
						isCommitAnyFileSelected={(commitId) =>
							isCommitAnyFileSelected(selectedBranchResolvedName, commitId)
						}
						isCommitFileSelected={(commitId, path) =>
							isCommitFileSelected(selectedBranchResolvedName, commitId, path)
						}
						toggleCommitSelection={(commitId) =>
							toggleCommitSelection(selectedBranchResolvedName, commitId)
						}
						toggleCommitFileSelection={(commitId, path) =>
							toggleCommitFileSelection(selectedBranchResolvedName, commitId, path)
						}
					/>
				)}
			</div>

			{selection != null && (
				<div>
					<Suspense fallback={<div>Loading diff…</div>}>
						{selection.commitId !== undefined ? (
							selection.path !== undefined ? (
								<SelectedCommitFileDiff
									projectId={id}
									commitId={selection.commitId}
									path={selection.path}
								/>
							) : (
								<SelectedCommitDiff projectId={id} commitId={selection.commitId} />
							)
						) : selectedBranchRef !== null ? (
							<SelectedBranchDiff projectId={id} branch={selectedBranchRef} />
						) : (
							<div>No branch diff available.</div>
						)}
					</Suspense>
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
