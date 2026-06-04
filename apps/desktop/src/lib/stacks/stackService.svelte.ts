import { getBranchNameFromRef } from "$lib/branches/branchUtils";
import { newPushFeature } from "$lib/config/uiFeatureFlags";
import { sortLikeFileTree } from "$lib/files/filetreeV3";
import { showToast } from "$lib/notifications/toasts";
import {
	selectWorkspaceStackById,
	selectWorkspaceStackDetails,
} from "$lib/stacks/headInfoAdapters";
import {
	changesSelectors,
	commitSelectors,
	selectChangesByPaths,
	stackSelectors,
	type BranchPushResult,
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
import type { BackendApi } from "$lib/state/backendApi";
import type { AppDispatch } from "$lib/state/clientState.svelte";
import type { AbsorptionTarget, DiffSpec, Stack } from "@gitbutler/but-sdk";

export { REJECTTION_REASONS } from "$lib/stacks/stackEndpoints";

type AmendCommitRequest = {
	projectId: string;
	stackId: string;
	commitId: string;
	worktreeChanges: DiffSpec[];
	dryRun: boolean;
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
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => stackSelectors.selectAll(workspaceDetails.stacks),
			},
		);
	}

	async fetchStacks(projectId: string) {
		return await this.backendApi.endpoints.workspaceDetails.fetch(
			{ projectId },
			{
				transform: (workspaceDetails) => stackSelectors.selectAll(workspaceDetails.stacks),
			},
		);
	}

	stackAt(projectId: string, index: number) {
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => stackSelectors.selectNth(workspaceDetails.stacks, index),
			},
		);
	}

	stackById(projectId: string, id: string) {
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => selectWorkspaceStackById(workspaceDetails, id) ?? null,
			},
		);
	}

	defaultBranch(projectId: string, stackId?: string) {
		if (!stackId) return null;
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) =>
					selectWorkspaceStackById(workspaceDetails, stackId)?.segments.at(0)?.refName
						?.displayName ?? null,
			},
		);
	}

	branchDetails(projectId: string, stackId: string | undefined, branchName?: string) {
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId, branchName);
					return branchName
						? details?.segments.find((segment) => segment.refName?.displayName === branchName)
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
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId);
					return details?.segments ?? [];
				},
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
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId, name);
					if (!details) return;
					const names = details.segments.map((segment) => segment.refName?.displayName);
					const index = names.indexOf(name);
					if (index === -1) return;

					const relativeName = names[index + offset];
					if (!relativeName) return;

					return details.segments.find((segment) => segment.refName?.displayName === relativeName);
				},
			},
		);
	}

	commits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId, branchName);
					return details?.segments.find((segment) => segment.refName?.displayName === branchName)
						?.commits;
				},
			},
		);
	}

	fetchCommits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.backendApi.endpoints.workspaceDetails.fetch(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId, branchName);
					return details?.segments.find((segment) => segment.refName?.displayName === branchName)
						?.commits;
				},
			},
		);
	}

	async fetchStackById(projectId: string, stackId: string) {
		return await this.backendApi.endpoints.workspaceDetails.fetch(
			{ projectId },
			{
				transform: (workspaceDetails) => selectWorkspaceStackById(workspaceDetails, stackId),
			},
		);
	}

	async fetchBranches(projectId: string, stackId: string) {
		return await this.backendApi.endpoints.workspaceDetails.fetch(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId);
					return details?.segments ?? [];
				},
			},
		);
	}

	allLocalCommits(projectId: string) {
		const details = $derived(
			this.backendApi.endpoints.workspaceDetails.useQuery(
				{ projectId },
				{
					transform: (workspaceDetails) => {
						const stacks = stackSelectors.selectAll(workspaceDetails.stacks);
						const stackIds = stacks.map((stack) => stack.id).filter(isDefined);
						const detailsData = stackIds
							.map((stackId) => workspaceDetails.stackDetails[stackId])
							.filter(isDefined);
						return {
							stackIds,
							detailsData,
							allCommits: detailsData.flatMap((details) =>
								commitSelectors.selectAll(details.commits),
							),
							allBranches: detailsData.flatMap((details) =>
								details.segments.map((segment) => segment.refName?.displayName).filter(isDefined),
							),
							allBaseCommitShas: detailsData.flatMap((details) =>
								details.segments
									.map((segment) => segment.base ?? details.stack.base)
									.filter(isDefined),
							),
						};
					},
				},
			),
		);
		const stackIds = $derived(details.response?.stackIds ?? []);
		const detailsData = $derived(details.response?.detailsData ?? []);
		const allCommits = $derived(details.response?.allCommits ?? []);
		const allBranches = $derived(details.response?.allBranches ?? []);
		const allBaseCommitShas = $derived(details.response?.allBaseCommitShas ?? []);

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

		// Tracks the previous stack per stackId for amend detection.
		// A plain object is used so that entries for removed stacks are naturally
		// dropped each time the snapshot is replaced (no manual cleanup needed).
		// Scoped here rather than on the service instance so it is tied to the
		// lifetime of this project session and not shared across project switches.
		let prevInfoSnapshot: Record<string, Stack> = {};

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

			const nextSnapshot: Record<string, Stack> = {};
			stackIds.forEach((stackId, i) => {
				const stack = detailsData[i]?.stack;
				if (!stack) return;
				// Only run when the Stack object is actually new (different reference).
				// During a re-fetch, RTK Query keeps the cached data object unchanged while
				// isFetching=true, so stack === prevInfo. Running updateStackSelection in
				// that window would see stale commit SHAs while selection.commitId may already
				// hold the new SHA (set by the caller), incorrectly treating the amend as a
				// deletion and clearing the drawer.
				const prevInfo = prevInfoSnapshot[stackId];
				if (stack !== prevInfo) {
					updateStackSelection(this.uiState, stackId, stack, prevInfo);
				}
				nextSnapshot[stackId] = stack;
			});
			prevInfoSnapshot = nextSnapshot;
		});

		return reactive(() => ({
			branches: allBranches,
			commits: allCommits,
		}));
	}

	commitById(projectId: string, stackId: string | undefined, commitId: string) {
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId);
					return (
						(details &&
							(commitSelectors.selectById(details.commits, commitId) ??
								upstreamCommitSelectors.selectById(details.upstreamCommits, commitId))) ??
						undefined
					);
				},
			},
		);
	}

	commitsByIds(projectId: string, stackId: string | undefined, commitIds: string[]) {
		return this.backendApi.endpoints.workspaceDetails.useQuery(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId);
					if (!details) return [];
					const commitDetails = commitIds.map((id) => {
						return (
							commitSelectors.selectById(details.commits, id) ??
							upstreamCommitSelectors.selectById(details.upstreamCommits, id)
						);
					});
					return commitDetails.filter(isDefined);
				},
			},
		);
	}

	fetchCommitById(projectId: string, stackId: string, commitId: string) {
		return this.backendApi.endpoints.workspaceDetails.fetch(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId);
					return (
						details &&
						(commitSelectors.selectById(details.commits, commitId) ??
							upstreamCommitSelectors.selectById(details.upstreamCommits, commitId))
					);
				},
			},
		);
	}

	fetchCommitsByIds(projectId: string, stackId: string, commitIds: string[]) {
		return this.backendApi.endpoints.workspaceDetails.fetch(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId);
					if (!details) return [];
					const commitDetails = commitIds.map((id) => {
						return (
							commitSelectors.selectById(details.commits, id) ??
							upstreamCommitSelectors.selectById(details.upstreamCommits, id)
						);
					});
					return commitDetails.filter(isDefined);
				},
			},
		);
	}

	get pushStack() {
		const options = {
			sideEffect: (result: BranchPushResult) => {
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
				if (code === "GitForcePushProtection") {
					throw commandError;
				}
				const reason =
					code === "ProjectGitAuth" ? "an authentication failure" : "an unforeseen error";
				showToast({
					title: "Git push failed",
					message: `Your branch cannot be pushed due to ${reason}.\n\nPlease check our [documentation](https://docs.gitbutler.com/troubleshooting/fetch-push)\non fetching and pushing for ways to resolve the problem.`,
					error: message,
					style: "warning",
				});
			},
			throwSilentError: true,
		};

		if (get(newPushFeature)) {
			return this.backendApi.endpoints.pushWorkspaceBranchAndAncestors.useMutation(options);
		} else {
			return this.backendApi.endpoints.pushStack.useMutation(options);
		}
	}

	createCommit() {
		return this.backendApi.endpoints.commitCreate.useMutation();
	}

	get createCommitMutation() {
		return this.backendApi.endpoints.commitCreate.mutate;
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
	branchChanges(args: { projectId: string; branch: string }) {
		return this.backendApi.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				branch: args.branch,
			},
			{
				transform: (result) => ({
					changes: sortLikeFileTree(changesSelectors.selectAll(result.changes)),
					stats: result.stats,
				}),
			},
		);
	}

	branchChange(args: { projectId: string; branch: string; path: string }) {
		return this.backendApi.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				branch: args.branch,
			},
			{ transform: (result) => changesSelectors.selectById(result.changes, args.path) },
		);
	}

	async branchChangesByPaths(args: { projectId: string; branch: string; paths: string[] }) {
		const result = await this.backendApi.endpoints.branchChanges.fetch(
			{
				projectId: args.projectId,
				branch: args.branch,
			},
			{ transform: (result) => selectChangesByPaths(result.changes, args.paths) },
		);
		return result || [];
	}

	get updateCommitMessage() {
		return this.backendApi.endpoints.updateCommitMessage.useMutation();
	}

	get newBranch() {
		return this.backendApi.endpoints.newBranch.useMutation();
	}

	async uncommit(args: { projectId: string; stackId?: string; commitIds: string[] }) {
		const result = await this.backendApi.endpoints.uncommit.mutate(args);
		if (args.stackId) {
			const selection = this.uiState.lane(args.stackId).selection;
			if (selection.current?.commitId && args.commitIds.includes(selection.current.commitId)) {
				selection.set(undefined);
			}
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

	async moveChangesBetweenCommits(args: {
		projectId: string;
		changes: DiffSpec[];
		sourceCommitId: string;
		sourceStackId: string;
		destinationCommitId: string;
		destinationStackId: string;
		dryRun: boolean;
	}) {
		return await this.backendApi.endpoints.commitMoveChangesBetween.mutate({
			projectId: args.projectId,
			changes: args.changes,
			sourceCommitId: args.sourceCommitId,
			destinationCommitId: args.destinationCommitId,
			dryRun: args.dryRun,
		});
	}

	async uncommitChanges(args: {
		projectId: string;
		changes: DiffSpec[];
		commitId: string;
		stackId: string;
		assignTo?: string;
		dryRun: boolean;
	}) {
		return await this.backendApi.endpoints.commitUncommitChanges.mutate({
			projectId: args.projectId,
			changes: args.changes,
			commitId: args.commitId,
			assignTo: args.assignTo,
			dryRun: args.dryRun,
		});
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

	get commitMove() {
		return this.backendApi.endpoints.commitMove.mutate;
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
		const [amendCommit, amendCommitQuery] = this.backendApi.endpoints.commitAmend.useMutation();
		return [
			(args: AmendCommitRequest) =>
				amendCommit({
					projectId: args.projectId,
					commitId: args.commitId,
					worktreeChanges: args.worktreeChanges,
					dryRun: args.dryRun,
				}),
			amendCommitQuery,
		] as const;
	}

	get amendCommitMutation() {
		return (args: AmendCommitRequest) =>
			this.backendApi.endpoints.commitAmend.mutate({
				projectId: args.projectId,
				commitId: args.commitId,
				worktreeChanges: args.worktreeChanges,
				dryRun: args.dryRun,
			});
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
		const allCommits = await this.backendApi.endpoints.workspaceDetails.fetch(
			{ projectId },
			{
				transform: (workspaceDetails) => {
					const details = selectWorkspaceStackDetails(workspaceDetails, stackId, branchName);
					return details?.segments.find((segment) => segment.refName?.displayName === branchName)
						?.commits;
				},
			},
		);

		if (!allCommits) return;
		const localCommits = allCommits.filter((commit) => commit.state.type !== "Integrated");

		if (localCommits.length <= 1) return;

		const targetCommit = localCommits.at(-1)!;
		const squashCommits = localCommits.slice(0, -1);

		await this.squashCommits({
			projectId,
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

	async fetchAbsorbPlan(projectId: string, target: AbsorptionTarget) {
		return await this.backendApi.endpoints.absorbPlan.fetch({ projectId, target });
	}
}
