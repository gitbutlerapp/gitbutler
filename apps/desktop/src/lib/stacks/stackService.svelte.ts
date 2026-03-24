import { getBranchNameFromRef } from "$lib/branches/branchUtils";
import { useNewRebaseEngine } from "$lib/config/uiFeatureFlags";
import { sortLikeFileTree } from "$lib/files/filetreeV3";
import { showToast } from "$lib/notifications/toasts";
import {
	branchDetailsSelectors,
	changesSelectors,
	commitSelectors,
	selectChangesByPaths,
	stackSelectors,
	upstreamCommitSelectors,
} from "$lib/stacks/stackEndpoints";
import {
	replaceBranchInExclusiveAction,
	replaceBranchInStackSelection,
	updateStaleProjectState,
	updateStackSelection,
} from "$lib/stacks/staleStateUpdaters";
import { invalidatesItem, invalidatesList, ReduxTag } from "$lib/state/tags";
import { type UiState } from "$lib/state/uiState.svelte";
import { InjectionToken } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import { isDefined } from "@gitbutler/ui/utils/typeguards";
import { get } from "svelte/store";
import type { ReduxError } from "$lib/error/reduxError";
import type { DefaultForgeFactory } from "$lib/forge/forgeFactory.svelte";
import type { StackDetails } from "$lib/stacks/stack";
import type { AppDispatch, BackendApi } from "$lib/state/clientState.svelte";
import type { HunkAssignment } from "@gitbutler/core/api";

export type {
	BranchParams,
	BranchPushResult,
	CreateCommitOutcome,
	CreateCommitRequest,
	CreateCommitRequestWorktreeChanges,
	RejectionReason,
	RelativeTo,
	SeriesIntegrationStrategy,
} from "$lib/stacks/stackEndpoints";
export { REJECTTION_REASONS } from "$lib/stacks/stackEndpoints";

const PUSH_ERROR_REASONS: Record<string, string> = {
	["errors.git.authentication"]: "an authentication failure",
	["errors.git.force_push_protection"]: "force push protection",
};

export const STACK_SERVICE = new InjectionToken<StackService>("StackService");

export class StackService {
	constructor(
		private backendApi: BackendApi,
		private dispatch: AppDispatch,
		private forgeFactory: DefaultForgeFactory,
		private uiState: UiState,
	) {}

	stacks(projectId: string) {
		return this.backendApi.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectAll(stacks),
			},
		);
	}

	async fetchStacks(projectId: string) {
		return await this.backendApi.endpoints.stacks.fetch(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectAll(stacks),
			},
		);
	}

	stackAt(projectId: string, index: number) {
		return this.backendApi.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectNth(stacks, index),
			},
		);
	}

	stackById(projectId: string, id: string) {
		return this.backendApi.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, id) ?? null,
			},
		);
	}

	allStackById(projectId: string, id: string) {
		return this.backendApi.endpoints.stacks.useQuery(
			{ projectId, all: true },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, id) ?? null,
			},
		);
	}

	defaultBranch(projectId: string, stackId?: string) {
		if (!stackId) return null;
		return this.backendApi.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, stackId)?.heads[0]?.name ?? null,
			},
		);
	}

	branchDetails(projectId: string, stackId: string | undefined, branchName?: string) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => {
					return branchName
						? branchDetailsSelectors.selectById(branchDetails, branchName)
						: undefined;
				},
			},
		);
	}

	get newStack() {
		return this.backendApi.endpoints.createStack.useMutation();
	}

	get newStackMutation() {
		return this.backendApi.endpoints.createStack.mutate;
	}

	get updateStackOrder() {
		return this.backendApi.endpoints.updateStackOrder.mutate;
	}

	branches(projectId: string, stackId?: string) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{ transform: ({ branchDetails }) => branchDetailsSelectors.selectAll(branchDetails) },
		);
	}

	branchAt(projectId: string, stackId: string | undefined, index: number) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ stackInfo }) => stackInfo.branchDetails[index],
			},
		);
	}

	/** Returns the parent of the branch specified by the provided name */
	branchParentByName(projectId: string, stackId: string | undefined, name: string) {
		return this.branchRelativeByName(projectId, stackId, name, 1);
	}

	/** Returns the child of the branch specified by the provided name */
	branchChildByName(projectId: string, stackId: string | undefined, name: string) {
		return this.branchRelativeByName(projectId, stackId, name, -1);
	}

	private branchRelativeByName(
		projectId: string,
		stackId: string | undefined,
		name: string,
		offset: number,
	) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ stackInfo, branchDetails }) => {
					const names = stackInfo.branchDetails.map((branch) => branch.name);
					const index = names.indexOf(name);
					if (index === -1) return;

					const relativeName = names[index + offset];
					if (!relativeName) return;

					return branchDetailsSelectors.selectById(branchDetails, relativeName);
				},
			},
		);
	}

	branchByName(projectId: string, stackId: string | undefined, name: string) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{ transform: ({ branchDetails }) => branchDetailsSelectors.selectById(branchDetails, name) },
		);
	}

	commits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits,
			},
		);
	}

	fetchCommits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.backendApi.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits,
			},
		);
	}

	async fetchStackById(projectId: string, stackId: string) {
		return await this.backendApi.endpoints.stacks.fetch(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, stackId),
			},
		);
	}

	async fetchBranches(projectId: string, stackId: string) {
		return await this.backendApi.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => branchDetailsSelectors.selectAll(branchDetails),
			},
		);
	}

	commitAt(projectId: string, stackId: string | undefined, branchName: string, index: number) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits[index] ?? null,
			},
		);
	}

	allLocalCommits(projectId: string) {
		const stacks = $derived(this.stacks(projectId));
		const stackIds = $derived(stacks.response?.map((s) => s.id).filter(isDefined) || []);
		const args = $derived(stackIds?.map((stackId) => ({ projectId, stackId })));
		const details = $derived(
			this.backendApi.endpoints.stackDetails.useQueries(args, {
				transform: ({ commits, stackInfo }) => ({
					commits: commitSelectors.selectAll(commits),
					branches: stackInfo.branchDetails.map((b) => b.name),
					baseCommitShas: stackInfo.branchDetails.map((b) => b.baseCommit),
					stackInfo,
				}),
			}),
		);
		const detailsData = $derived(details.current);
		const allCommits = $derived(detailsData.flatMap((d) => d.data?.commits ?? []));
		const allBranches = $derived(detailsData.flatMap((d) => d.data?.branches ?? []));
		const allBaseCommitShas = $derived(detailsData.flatMap((d) => d.data?.baseCommitShas ?? []));

		$effect(() => {
			updateStaleProjectState(
				this.uiState,
				projectId,
				stackIds,
				allBranches,
				allCommits.map((c) => c.id),
				allBaseCommitShas,
			);
		});

		// Tracks the previous StackDetails per stackId for amend detection.
		// A plain object is used so that entries for removed stacks are naturally
		// dropped each time the snapshot is replaced (no manual cleanup needed).
		// Scoped here rather than on the service instance so it is tied to the
		// lifetime of this project session and not shared across project switches.
		let prevInfoSnapshot: Record<string, StackDetails> = {};

		// Having lots of commits in a GitButler workspace is an extreme edge case that
		// is not a realistic usage scenario. We skip selection repair at this scale to
		// avoid degrading UI responsiveness — the linear index scans in
		// updateStackSelection across thousands of commits add up when the effect
		// fires on every stack-details refresh.
		const STALE_SELECTION_COMMIT_LIMIT = 1000;

		$effect(() => {
			if (allCommits.length > STALE_SELECTION_COMMIT_LIMIT) {
				console.warn(
					`Skipping stale selection detection: commit count (${allCommits.length}) exceeds limit (${STALE_SELECTION_COMMIT_LIMIT}).`,
				);
				return;
			}

			const nextSnapshot: Record<string, StackDetails> = {};
			stackIds.forEach((stackId, i) => {
				const stackInfo = detailsData[i]?.data?.stackInfo;
				if (!stackInfo) return;
				// Only run when the StackDetails object is actually new (different reference).
				// During a re-fetch, RTK Query keeps the cached data object unchanged while
				// isFetching=true, so stackInfo === prevInfo. Running updateStackSelection in
				// that window would see stale commit SHAs while selection.commitId may already
				// hold the new SHA (set by the caller), incorrectly treating the amend as a
				// deletion and clearing the drawer.
				const prevInfo = prevInfoSnapshot[stackId];
				if (stackInfo !== prevInfo) {
					updateStackSelection(this.uiState, stackId, stackInfo, prevInfo);
				}
				nextSnapshot[stackId] = stackInfo;
			});
			prevInfoSnapshot = nextSnapshot;
		});

		return reactive(() => ({
			branches: allBranches,
			commits: allCommits,
		}));
	}

	commitById(projectId: string, stackId: string | undefined, commitId: string) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) =>
					commitSelectors.selectById(commits, commitId) ??
					upstreamCommitSelectors.selectById(upstreamCommits, commitId),
			},
		);
	}

	commitsByIds(projectId: string, stackId: string | undefined, commitIds: string[]) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) => {
					const commitDetails = commitIds.map((id) => {
						return (
							commitSelectors.selectById(commits, id) ??
							upstreamCommitSelectors.selectById(upstreamCommits, id)
						);
					});
					return commitDetails.filter(isDefined);
				},
			},
		);
	}

	fetchCommitById(projectId: string, stackId: string, commitId: string) {
		return this.backendApi.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) =>
					commitSelectors.selectById(commits, commitId) ??
					upstreamCommitSelectors.selectById(upstreamCommits, commitId),
			},
		);
	}

	fetchCommitsByIds(projectId: string, stackId: string, commitIds: string[]) {
		return this.backendApi.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) => {
					const commitDetails = commitIds.map((id) => {
						return (
							commitSelectors.selectById(commits, id) ??
							upstreamCommitSelectors.selectById(upstreamCommits, id)
						);
					});
					return commitDetails.filter(isDefined);
				},
			},
		);
	}

	upstreamCommits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.upstreamCommits,
			},
		);
	}

	upstreamCommitAt(projectId: string, stackId: string, branchName: string, index: number) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.upstreamCommits[index] ??
					null,
			},
		);
	}

	fetchUpstreamCommitById(projectId: string, stackId: string, commitId: string) {
		return this.backendApi.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ upstreamCommits }) =>
					upstreamCommitSelectors.selectById(upstreamCommits, commitId),
			},
		);
	}

	get pushStack() {
		return this.backendApi.endpoints.pushStack.useMutation({
			sideEffect: (result, _) => {
				// Timeout to accommodate eventual consistency.
				setTimeout(() => {
					const invalidations = [invalidatesList(ReduxTag.PullRequests)];

					if (result) {
						const upstreamBranchNames = result.branchToRemote
							.map(([_, refname]) => getBranchNameFromRef(refname, result.remote))
							.filter(isDefined);
						for (const name of upstreamBranchNames) {
							invalidations.push(invalidatesItem(ReduxTag.Checks, name));
						}
					}

					this.forgeFactory.invalidate(invalidations);
				}, 2000);
			},
			onError: (commandError: ReduxError) => {
				const { code, message } = commandError;
				if (code === "errors.git.force_push_protection") {
					throw commandError;
				}
				const reason = PUSH_ERROR_REASONS[code ?? ""] ?? "an unforeseen error";
				showToast({
					title: "Git push failed",
					message: `Your branch cannot be pushed due to ${reason}.\n\nPlease check our [documentation](https://docs.gitbutler.com/troubleshooting/fetch-push)\non fetching and pushing for ways to resolve the problem.`,
					error: message,
					style: "danger",
				});
			},
			throwSilentError: true,
		});
	}

	createCommit() {
		if (get(useNewRebaseEngine)) {
			return this.backendApi.endpoints.commitCreate.useMutation();
		} else {
			return this.backendApi.endpoints.legacyCreateCommit.useMutation();
		}
	}

	get createCommitMutation() {
		if (get(useNewRebaseEngine)) {
			return this.backendApi.endpoints.commitCreate.mutate;
		} else {
			return this.backendApi.endpoints.legacyCreateCommit.mutate;
		}
	}

	filePathsChangedInCommits(projectId: string, commitIds: string[]) {
		const params = commitIds.map((commitId) => ({
			projectId,
			commitId,
		}));
		return this.backendApi.endpoints.commitDetails.useQueries(params, {
			transform: (results) => {
				return results.changes.ids;
			},
		});
	}

	commitChanges(projectId: string, commitId: string) {
		return this.backendApi.endpoints.commitDetails.useQuery(
			{ projectId, commitId },
			{
				transform: (result) => ({
					changes: sortLikeFileTree(changesSelectors.selectAll(result.changes)),
					stats: result.stats,
					conflictEntries: result.conflictEntries,
				}),
			},
		);
	}

	fetchCommitChanges(projectId: string, commitId: string) {
		return this.backendApi.endpoints.commitDetails.fetch(
			{ projectId, commitId },
			{
				transform: (result) => ({
					changes: sortLikeFileTree(changesSelectors.selectAll(result.changes)),
					stats: result.stats,
					conflictEntries: result.conflictEntries,
				}),
			},
		);
	}

	commitChange(projectId: string, commitId: string, path: string) {
		return this.backendApi.endpoints.commitDetails.useQuery(
			{ projectId, commitId },
			{ transform: (result) => changesSelectors.selectById(result.changes, path) },
		);
	}

	async commitChangesByPaths(projectId: string, commitId: string, paths: string[]) {
		const result = await this.backendApi.endpoints.commitDetails.fetch(
			{ projectId, commitId },
			{ transform: (result) => selectChangesByPaths(result.changes, paths) },
		);
		return result || [];
	}

	commitDetails(projectId: string, commitId: string) {
		return this.backendApi.endpoints.commitDetails.useQuery(
			{ projectId, commitId },
			{ transform: (result) => result.details },
		);
	}

	fetchCommitDetails(projectId: string, commitId: string) {
		return this.backendApi.endpoints.commitDetails.fetch(
			{ projectId, commitId },
			{ transform: (result) => result.details },
		);
	}

	/**
	 * Gets the changes for a given branch.
	 */
	branchChanges(args: { projectId: string; stackId?: string; branch: string }) {
		return this.backendApi.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				branch: args.branch,
				stackId: args.stackId,
			},
			{
				transform: (result) => ({
					changes: sortLikeFileTree(changesSelectors.selectAll(result.changes)),
					stats: result.stats,
				}),
			},
		);
	}

	branchChange(args: { projectId: string; stackId?: string; branch: string; path: string }) {
		return this.backendApi.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branch: args.branch,
			},
			{ transform: (result) => changesSelectors.selectById(result.changes, args.path) },
		);
	}

	async branchChangesByPaths(args: {
		projectId: string;
		stackId?: string;
		branch: string;
		paths: string[];
	}) {
		const result = await this.backendApi.endpoints.branchChanges.fetch(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branch: args.branch,
			},
			{ transform: (result) => selectChangesByPaths(result.changes, args.paths) },
		);
		return result || [];
	}

	get updateCommitMessage() {
		if (get(useNewRebaseEngine)) {
			return this.backendApi.endpoints.updateCommitMessage.useMutation();
		} else {
			return this.backendApi.endpoints.legacyUpdateCommitMessage.useMutation();
		}
	}

	get newBranch() {
		return this.backendApi.endpoints.newBranch.useMutation();
	}

	async uncommit(args: {
		projectId: string;
		stackId: string;
		branchName: string;
		commitId: string;
	}) {
		const result = await this.backendApi.endpoints.uncommit.mutate(args);
		const selection = this.uiState.lane(args.stackId).selection;
		if (args.commitId === selection.current?.commitId) {
			selection.set(undefined);
		}
		return result;
	}

	get insertBlankCommit() {
		return this.backendApi.endpoints.insertBlankCommit;
	}

	get unapply() {
		return this.backendApi.endpoints.unapply.mutate;
	}

	get discardChanges() {
		return this.backendApi.endpoints.discardChanges.mutate;
	}

	get moveChangesBetweenCommits() {
		if (get(useNewRebaseEngine)) {
			return this.backendApi.endpoints.commitMoveChangesBetween.mutate;
		} else {
			return this.backendApi.endpoints.legacyMoveChangesBetweenCommits.mutate;
		}
	}

	get uncommitChanges() {
		if (get(useNewRebaseEngine)) {
			return this.backendApi.endpoints.commitUncommitChanges.mutate;
		} else {
			return this.backendApi.endpoints.legacyUncommitChanges.mutate;
		}
	}

	get stashIntoBranch() {
		return this.backendApi.endpoints.stashIntoBranch.mutate;
	}

	get updateBranchPrNumber() {
		return this.backendApi.endpoints.updateBranchPrNumber.mutate;
	}

	get updateBranchName() {
		return this.backendApi.endpoints.updateBranchName.useMutation({
			sideEffect: (_, args) => {
				// Immediately update the selection and the exclusive action.
				const laneState = this.uiState.lane(args.laneId);
				const projectState = this.uiState.project(args.projectId);
				const exclusiveAction = projectState.exclusiveAction.current;
				const previousSelection = laneState.selection.current;

				if (previousSelection) {
					const updatedSelection = replaceBranchInStackSelection(
						previousSelection,
						args.branchName,
						args.newName,
					);
					laneState.selection.set(updatedSelection);
				}

				if (exclusiveAction) {
					const updatedExclusiveAction = replaceBranchInExclusiveAction(
						exclusiveAction,
						args.branchName,
						args.newName,
					);
					projectState.exclusiveAction.set(updatedExclusiveAction);
				}
			},
			onError: (_, args) => {
				const state = this.uiState.lane(args.laneId);
				const previewOpen = state.selection.current?.previewOpen ?? false;
				state.selection.set({
					branchName: args.branchName,
					previewOpen,
				});
			},
		});
	}

	get removeBranch() {
		return this.backendApi.endpoints.removeBranch.useMutation();
	}

	get reorderStack() {
		return this.backendApi.endpoints.reorderStack.mutate;
	}

	get moveCommit() {
		return this.backendApi.endpoints.moveCommit.mutate;
	}

	get moveBranch() {
		return this.backendApi.endpoints.moveBranch.mutate;
	}

	get tearOffBranch() {
		return this.backendApi.endpoints.tearOffBranch.mutate;
	}

	get integrateUpstreamCommits() {
		return this.backendApi.endpoints.integrateUpstreamCommits.useMutation();
	}

	initialIntegrationSteps(projectId: string, stackId: string | undefined, branchName: string) {
		return this.backendApi.endpoints.getInitialIntegrationSteps.useQuery({
			projectId,
			stackId,
			branchName,
		});
	}

	get integrateBranchWithSteps() {
		return this.backendApi.endpoints.integrateBranchWithSteps.useMutation();
	}

	get createVirtualBranchFromBranch() {
		return this.backendApi.endpoints.createVirtualBranchFromBranch.mutate;
	}

	get deleteLocalBranch() {
		return this.backendApi.endpoints.deleteLocalBranch.mutate;
	}

	get squashCommits() {
		return this.backendApi.endpoints.squashCommits.mutate;
	}

	get amendCommit() {
		if (get(useNewRebaseEngine)) {
			return this.backendApi.endpoints.commitAmend.useMutation();
		} else {
			return this.backendApi.endpoints.legacyAmendCommit.useMutation();
		}
	}

	get amendCommitMutation() {
		if (get(useNewRebaseEngine)) {
			return this.backendApi.endpoints.commitAmend.mutate;
		} else {
			return this.backendApi.endpoints.legacyAmendCommit.mutate;
		}
	}

	/** Squash all the commits in a branch together */
	async squashAllCommits({
		projectId,
		stackId,
		branchName,
	}: {
		projectId: string;
		stackId: string;
		branchName: string;
	}) {
		const allCommits = await this.backendApi.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits,
			},
		);

		if (!allCommits) return;
		const localCommits = allCommits.filter((commit) => commit.state.type !== "Integrated");

		if (localCommits.length <= 1) return;

		const targetCommit = localCommits.at(-1)!;
		const squashCommits = localCommits.slice(0, -1);

		await this.squashCommits({
			projectId,
			stackId,
			sourceCommitIds: squashCommits.map((commit) => commit.id),
			targetCommitId: targetCommit.id,
		});
	}

	newBranchName(projectId: string) {
		return this.backendApi.endpoints.newBranchName.useQuery({ projectId }, { forceRefetch: true });
	}

	async fetchNewBranchName(projectId: string) {
		return await this.backendApi.endpoints.newBranchName.fetch(
			{ projectId },
			{ forceRefetch: true },
		);
	}

	isBranchConflicted(projectId: string, stackId: string, branchName: string) {
		return this.backendApi.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => {
					const branch = branchDetailsSelectors.selectById(branchDetails, branchName);
					return branch?.isConflicted ?? false;
				},
			},
		);
	}

	async normalizeBranchName(name: string) {
		return await this.backendApi.endpoints.normalizeBranchName.fetch(
			{ name },
			{ forceRefetch: true },
		);
	}

	/**
	 * Note: This is specifically for looking up branches outside of
	 * a stacking context. You almost certainly want `stackDetails`
	 */
	unstackedBranchDetails(projectId: string, branchName: string, remote?: string) {
		return this.backendApi.endpoints.unstackedBranchDetails.useQuery(
			{ projectId, branchName, remote },
			{ transform: (result) => result.branchDetails },
		);
	}

	unstackedCommits(projectId: string, branchName: string, remote?: string) {
		return this.backendApi.endpoints.unstackedBranchDetails.useQuery(
			{ projectId, branchName, remote },
			{
				transform: (data) => commitSelectors.selectAll(data.commits),
			},
		);
	}

	async fetchUnstackedCommits(projectId: string, branchName: string, remote?: string) {
		return await this.backendApi.endpoints.unstackedBranchDetails.fetch(
			{ projectId, branchName, remote },
			{
				transform: (data) => commitSelectors.selectAll(data.commits),
			},
		);
	}

	unstackedCommitById(projectId: string, branchName: string, commitId: string, remote?: string) {
		return this.backendApi.endpoints.unstackedBranchDetails.useQuery(
			{ projectId, branchName, remote },
			{ transform: ({ commits }) => commitSelectors.selectById(commits, commitId) },
		);
	}

	async targetCommits(projectId: string, lastCommitId: string | undefined, pageSize: number) {
		return await this.backendApi.endpoints.targetCommits.fetch(
			{ projectId, lastCommitId, pageSize },
			{
				forceRefetch: true,
				transform: (commits) => commitSelectors.selectAll(commits),
			},
		);
	}

	get splitBranch() {
		return this.backendApi.endpoints.splitBranch.useMutation();
	}

	get splitBranchMutation() {
		return this.backendApi.endpoints.splitBranch.mutate;
	}

	get splitBranchIntoDependentBranch() {
		return this.backendApi.endpoints.splitBranchIntoDependentBranch.useMutation();
	}

	invalidateStacksAndDetails() {
		this.dispatch(
			this.backendApi.util.invalidateTags([
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
			]),
		);
	}

	templates(projectId: string, forgeName: string) {
		return this.backendApi.endpoints.templates.useQuery({ projectId, forge: forgeName });
	}

	async template(projectId: string, forgeName: string, relativePath: string) {
		return await this.backendApi.endpoints.template.fetch({
			relativePath,
			projectId,
			forge: forgeName,
		});
	}

	get createReference() {
		return this.backendApi.endpoints.createReference.useMutation();
	}

	get absorb() {
		return this.backendApi.endpoints.absorb.useMutation();
	}

	async fetchAbsorbPlan(projectId: string, target: HunkAssignment.AbsorptionTarget) {
		return await this.backendApi.endpoints.absorbPlan.fetch({ projectId, target });
	}
}
