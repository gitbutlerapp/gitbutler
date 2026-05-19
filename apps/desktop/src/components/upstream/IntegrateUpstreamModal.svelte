<script lang="ts">
	import { BACKEND } from "$lib/backend";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import UnityConflictResolverModal from "$components/workspace/UnityConflictResolverModal.svelte";
	import { descriptionTitle } from "$lib/commits/commit";
	import { isUnityYamlPath } from "$lib/files/unityConflicts";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import {
		getBaseBranchResolution,
		stackFullyIntegrated,
		sortStatusInfoV3,
		getResolutionApproachV3,
		type StackStatusInfoV3,
		type StackStatusesWithBranchesV3,
	} from "$lib/upstream/types";
	import { UPSTREAM_INTEGRATION_SERVICE } from "$lib/upstream/upstreamIntegrationService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		getBooleanStorageItem,
		removeStorageItem,
		setBooleanStorageItem,
	} from "@gitbutler/shared/persisted";
	import {
		Badge,
		Button,
		IntegrationSeriesRow,
		Modal,
		FileListItem,
		Icon,
		Select,
		SelectItem,
		ScrollableContainer,
		type BranchShouldBeDeletedMap,
		TestId,
		AsyncButton,
	} from "@gitbutler/ui";
	import { tick } from "svelte";
	import { SvelteMap } from "svelte/reactivity";
	import type { PullRequest } from "$lib/forge/interface/types";
	import type {
		BaseBranchResolutionApproach,
		BranchStatus,
		Resolution,
		StackStatus,
	} from "@gitbutler/but-sdk";

	type OperationState = "inert" | "loading" | "completed";

	interface Props {
		projectId: string;
		onClose?: () => void;
	}

	const { projectId, onClose }: Props = $props();

	const upstreamIntegrationService = inject(UPSTREAM_INTEGRATION_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	// const forgeListingService = $derived(forge.current.listService);
	const backend = inject(BACKEND);
	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchQuery.response);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>("inert");
	const results = new SvelteMap<string, Resolution>();
	let statuses = $state<StackStatusInfoV3[]>([]);
	const baseResolutionOptions = [
		{ label: "Rebase", value: "rebase" as const },
		{ label: "Merge", value: "merge" as const },
		{ label: "Hard reset", value: "hardReset" as const },
	];
	let baseResolutionApproach = $state<BaseBranchResolutionApproach | undefined>();
	let targetCommitOid = $state<string | undefined>(undefined);
	let branchStatuses = $state<StackStatusesWithBranchesV3 | undefined>();
	let loadingStatuses = $state(false);
	let workspaceUpdateProgress = $state<WorkspaceUpdateProgress | undefined>();
	let gitOperationProgress = $state<GitOperationProgress | undefined>();
	let gitOperationStartedAt = $state<number | undefined>();
	let elapsedTick = $state(Date.now());
	let unityConflictModal = $state<UnityConflictResolverModal | undefined>();
	let incomingChangesExpanded = $state(true);
	let conflictsExpanded = $state(true);
	let selectedIncomingCommitId = $state<string | undefined>();
	let activeProgress = $derived(
		activeProgressPercent(workspaceUpdateProgress, gitOperationProgress),
	);
	let progressVisible = $derived(loadingStatuses || integratingUpstream === "loading");
	// const stackService = getContext(StackService);
	// let appliedBranches = $state<string[]>();
	// Any PRs belonging to applied branches that have been merged
	let filteredReviews = $state<PullRequest[]>([]);
	const reviewMap = $derived(new Map(filteredReviews?.map((r) => [r.sourceBranch, r])));

	const isDivergedResolved = $derived(base?.targetShaAheadOfRef && !baseResolutionApproach);
	const [integrateUpstream] = $derived(upstreamIntegrationService.integrateUpstream());

	type WorkspaceUpdateProgress = {
		direction: string;
		currentFile: number;
		totalFiles: number;
		fileDownloadedBytes: number;
		fileTotalBytes: number;
		progressPercent: number;
		bytesPerSecond?: number;
		path: string;
	};

	type GitOperationProgress = {
		operation: string;
		phase: string;
		phaseLabel: string;
		elapsedMs: number;
		path?: string;
		currentPath?: number;
		totalPaths?: number;
		bytesDone?: number;
		bytesTotal?: number;
		bytesPerSecond?: number;
		lfsDirection?: string;
		detail?: string;
	};

	$effect(() => {
		if (!progressVisible) {
			workspaceUpdateProgress = undefined;
			gitOperationProgress = undefined;
			gitOperationStartedAt = undefined;
			return;
		}

		const timer = window.setInterval(() => {
			elapsedTick = Date.now();
		}, 1000);
		const unlistenWorkspaceProgress = backend.listen<WorkspaceUpdateProgress>(
			`project://${projectId}/workspace_update_progress`,
			({ payload }) => {
				workspaceUpdateProgress = payload;
			},
		);
		const unlistenGitOperationProgress = backend.listen<GitOperationProgress>(
			`project://${projectId}/git_operation_progress`,
			({ payload }) => {
				if (payload.operation === "upstreamIntegration" || payload.operation === "upstreamStatus") {
					gitOperationProgress = payload;
					gitOperationStartedAt = Date.now() - payload.elapsedMs;
				}
			},
		);

		return () => {
			window.clearInterval(timer);
			void unlistenWorkspaceProgress();
			void unlistenGitOperationProgress();
		};
	});

	function someBranchesShouldNotBeDeleted(branchNames: string[]): boolean {
		for (const branchName of branchNames) {
			const key = getDontDeleteBranchStorageKey(branchName);
			const dontDelete = getBooleanStorageItem(key);
			if (dontDelete) {
				return true;
			}
		}
		return false;
	}

	$effect(() => {
		if (!modal?.imports.open) return;
		if (branchStatuses?.type !== "updatesRequired") {
			statuses = [];
			return;
		}

		const statusesTmp = [...branchStatuses.subject];
		statusesTmp.sort(sortStatusInfoV3);

		// Side effect, refresh results
		results.clear();
		for (const status of statusesTmp) {
			if (status.stack.id) {
				const dontDelete = someBranchesShouldNotBeDeleted(status.stack.heads.map((b) => b.name));

				results.set(status.stack.id, {
					stackId: status.stack.id,
					approach: getResolutionApproachV3(status),
					deleteIntegratedBranches: !dontDelete,
				});
			}
		}

		statuses = statusesTmp;
	});

	// Re-fetch upstream statuses if the target commit oid changes
	$effect(() => {
		if (!modal?.imports.open) return;
		if (targetCommitOid) {
			loadingStatuses = true;
			startLocalProgress(
				"upstreamStatus",
				"status",
				"Checking upstream status",
				"Computing update options for the selected target commit.",
			);
			void (async () => {
				try {
					await tick();
					branchStatuses = await upstreamIntegrationService.upstreamStatuses(
						projectId,
						targetCommitOid,
						setLocalProgress,
					);
				} finally {
					loadingStatuses = false;
				}
			})();
		}
	});

	// Resolve the target commit oid if the base branch diverged and the the resolution
	// approach is changed
	$effect(() => {
		if (!modal?.imports.open) return;
		if (base?.targetShaAheadOfRef && baseResolutionApproach) {
			upstreamIntegrationService
				.resolveUpstreamIntegrationMutation({
					projectId,
					resolutionApproach: baseResolutionApproach,
				})
				.then((result) => {
					targetCommitOid = result;
				});
		} else {
			// If there is no divergence we should set this to undefined.
			targetCommitOid = undefined;
		}
	});

	// async function setFilteredBranches(appliedBranches: string[]) {
	// 	if (!forgeListingService) return;

	// 	try {
	// 		// Fetch the base branch and the forge info to ensure we have the
	// 		// latest data We only need to (and want to) do this if we are also
	// 		// looking at the reviews.
	// 		//
	// 		// This is to handle the case where the reviews might dictacte that
	// 		// we should remove a branch, but we don't have the have the merge
	// 		// commit yet. If we were to handle a branch as "integrated" without
	// 		// the merge commit, files might disappear for a users working tree
	// 		// in a supprising way.
	// 		//
	// 		// We could query both of these simultaneously using Promise.all,
	// 		// but that is extra complexity that is not needed for now.
	// 		await baseBranchService.fetchFromRemotes(projectId);
	// 		const reviews = await forgeListingService.fetchByBranch(projectId, appliedBranches);

	// 		// Find the reviews that have a "mergedAt" timestamp
	// 		filteredReviews = reviews.filter((r) => !!r.mergedAt);
	// 	} catch (_e) {
	// 		// We don't really mind if this fails as additional bonus
	// 		// information.
	// 	}
	// }

	function getDontDeleteBranchStorageKey(branchName: string): string {
		return `integrate-upstream-modal:dont-delete-branch:${projectId}:${branchName}`;
	}

	function handleBaseResolutionSelection(value: BaseBranchResolutionApproach["type"]) {
		baseResolutionApproach = { type: value };
	}

	async function integrate() {
		integratingUpstream = "loading";
		workspaceUpdateProgress = undefined;
		startLocalProgress(
			"upstreamIntegration",
			"prepare",
			"Preparing upstream integration",
			"Git LFS hydration is deferred for this operation.",
		);
		await tick();
		const baseResolution = getBaseBranchResolution(
			targetCommitOid,
			baseResolutionApproach ?? { type: "hardReset" },
		);

		await integrateUpstream({
			projectId,
			resolutions: Array.from(results.values()),
			baseBranchResolution: baseResolution,
		});
		await baseBranchService.refreshBaseBranch(projectId);
		integratingUpstream = "completed";
		modal?.close();
	}

	function formatFileSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
		return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
	}

	function formatTransferSpeed(bytesPerSecond: number | undefined): string | undefined {
		if (bytesPerSecond === undefined) return undefined;
		return `${formatFileSize(bytesPerSecond)}/s`;
	}

	function formatProgressPercent(progressPercent: number): string {
		return `${Math.round(progressPercent)}%`;
	}

	function formatElapsed(ms: number | undefined): string | undefined {
		if (ms === undefined) return undefined;
		const seconds = Math.floor(ms / 1000);
		if (seconds < 60) return `${seconds}s`;
		const minutes = Math.floor(seconds / 60);
		const remainingSeconds = seconds % 60;
		return `${minutes}m ${remainingSeconds}s`;
	}

	function currentElapsedMs(progress: GitOperationProgress | undefined): number | undefined {
		if (!progress) return undefined;
		if (gitOperationStartedAt === undefined) return progress.elapsedMs;
		return Math.max(progress.elapsedMs, elapsedTick - gitOperationStartedAt);
	}

	function startLocalProgress(
		operation: string,
		phase: string,
		phaseLabel: string,
		detail?: string,
	) {
		gitOperationStartedAt = Date.now();
		elapsedTick = Date.now();
		gitOperationProgress = {
			operation,
			phase,
			phaseLabel,
			elapsedMs: 0,
			detail,
		};
	}

	function setLocalProgress(progress: { phase: string; phaseLabel: string; detail?: string }) {
		if (gitOperationStartedAt === undefined) {
			gitOperationStartedAt = Date.now();
		}
		elapsedTick = Date.now();
		gitOperationProgress = {
			operation: "upstreamStatus",
			...progress,
			elapsedMs: elapsedTick - gitOperationStartedAt,
		};
	}

	function activeProgressPercent(
		workspaceProgress: WorkspaceUpdateProgress | undefined,
		gitProgress: GitOperationProgress | undefined,
	): number | undefined {
		if (workspaceProgress) return workspaceProgress.progressPercent;
		if (
			gitProgress?.bytesDone !== undefined &&
			gitProgress.bytesTotal !== undefined &&
			gitProgress.bytesTotal > 0
		) {
			return (gitProgress.bytesDone / gitProgress.bytesTotal) * 100;
		}
		return undefined;
	}

	function workspaceUpdateStatusText(
		progress: WorkspaceUpdateProgress | undefined,
		gitProgress: GitOperationProgress | undefined = gitOperationProgress,
	): string {
		if (!progress) {
			const elapsed = formatElapsed(currentElapsedMs(gitProgress));
			const phase = gitProgress?.phaseLabel ?? "Preparing workspace update";
			const suffix = elapsed ? `Elapsed ${elapsed}.` : "Waiting for Git progress.";
			return `${phase}. ${suffix}`;
		}

		const speed = formatTransferSpeed(progress.bytesPerSecond);
		const progressLabel = `${formatProgressPercent(progress.progressPercent)} complete`;
		const fileLabel = `file ${progress.currentFile} of ${progress.totalFiles}`;
		return speed
			? `Downloading ${fileLabel} at ${speed}. ${progressLabel}.`
			: `Downloading ${fileLabel}. ${progressLabel}.`;
	}

	function workspaceUpdateTooltip(
		progress: WorkspaceUpdateProgress | undefined,
		gitProgress: GitOperationProgress | undefined = gitOperationProgress,
	): string {
		const status = workspaceUpdateStatusText(progress, gitProgress);
		if (!progress) {
			return gitProgress?.detail ? `${status} ${gitProgress.detail}` : status;
		}

		return `${status} Current path: ${progress.path}`;
	}

	function progressWidth(progressPercent: number | undefined): number {
		if (progressPercent === undefined) return 0;
		return Math.max(0, Math.min(100, progressPercent));
	}

	function shortSha(sha: string): string {
		return sha.slice(0, 7);
	}

	function changeStatusLabel(type: string): string {
		switch (type) {
			case "Addition":
				return "Added";
			case "Deletion":
				return "Deleted";
			case "Modification":
				return "Modified";
			case "Rename":
				return "Renamed";
			default:
				return type;
		}
	}

	// async function fetchAppliedBranches() {
	// 	const stacksResponse = await stackService.fetchStacks(projectId);
	// 	return stacksResponse.data?.flatMap((stack) => stack.heads.map((head) => head.name)) ?? [];
	// }

	export async function show() {
		integratingUpstream = "inert";
		loadingStatuses = true;
		branchStatuses = undefined;
		filteredReviews = [];
		startLocalProgress(
			"upstreamStatus",
			"stacks",
			"Loading workspace stacks",
			"Reading applied branches before upstream conflict analysis.",
		);
		await tick();
		modal?.show();
		// appliedBranches = await fetchAppliedBranches();
		// await setFilteredBranches(untrack(() => appliedBranches) ?? []); // TODO: Some day this will be made good
		try {
			branchStatuses = await upstreamIntegrationService.upstreamStatuses(
				projectId,
				targetCommitOid,
				setLocalProgress,
			);
		} finally {
			loadingStatuses = false;
		}
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		},
	};

	function branchStatusToRowEntry(
		associatedeReview: PullRequest | undefined,
		branchStatus: BranchStatus,
	): "integrated" | "conflicted" | "clear" {
		if (associatedeReview?.mergedAt !== undefined) {
			return "integrated";
		}

		if (branchStatus.type === "integrated") {
			return "integrated";
		}

		if (branchStatus.type === "conflicted") {
			return "conflicted";
		}

		return "clear";
	}

	function integrationRowSeries(
		stackStatus: StackStatus,
	): { name: string; status: "integrated" | "conflicted" | "clear" }[] {
		const statuses = stackStatus.branchStatuses.map((series) => {
			const associatedeReview = reviewMap.get(series.name);
			return {
				name: series.name,
				status: branchStatusToRowEntry(associatedeReview, series.status),
			};
		});

		statuses.reverse();

		return statuses;
	}
	function getBranchShouldBeDeletedMap(
		stackId: string,
		stackStatus: StackStatus,
	): BranchShouldBeDeletedMap {
		const branchShouldBeDeletedMap: BranchShouldBeDeletedMap = {};
		stackStatus.branchStatuses.forEach((branch) => {
			branchShouldBeDeletedMap[branch.name] = !!results.get(stackId)?.deleteIntegratedBranches;
		});
		return branchShouldBeDeletedMap;
	}

	function updateBranchShouldBeDeletedMap(
		stackId: string,
		branchNames: string[],
		shouldBeDeleted: boolean,
	): void {
		const result = results.get(stackId);
		if (!result) return;
		for (const branchName of branchNames) {
			const key = getDontDeleteBranchStorageKey(branchName);
			if (!shouldBeDeleted) {
				setBooleanStorageItem(key, true);
			} else {
				removeStorageItem(key);
			}
		}
		results.set(stackId, { ...result, deleteIntegratedBranches: shouldBeDeleted });
	}

	function integrationOptions(
		stackStatus: StackStatus,
	): { label: string; value: "rebase" | "unapply" | "merge" }[] {
		if (stackStatus.branchStatuses.length > 1) {
			return [
				{ label: "Rebase", value: "rebase" },
				{ label: "Stash", value: "unapply" },
			];
		} else {
			return [
				{ label: "Rebase", value: "rebase" },
				{ label: "Merge", value: "merge" },
				{ label: "Stash", value: "unapply" },
			];
		}
	}
</script>

{#snippet stackStatus(stackId: string, stackStatus: StackStatus)}
	{@const branchShouldBeDeletedMap = getBranchShouldBeDeletedMap(stackId, stackStatus)}
	<IntegrationSeriesRow
		testId={TestId.IntegrateUpstreamSeriesRow}
		series={integrationRowSeries(stackStatus)}
		{branchShouldBeDeletedMap}
		updateBranchShouldBeDeletedMap={(branchNames, shouldBeDeleted) =>
			updateBranchShouldBeDeletedMap(stackId, branchNames, shouldBeDeleted)}
	>
		{#if !stackFullyIntegrated(stackStatus) && results.get(stackId)}
			<Select
				value={results.get(stackId)!.approach.type}
				maxWidth={130}
				onselect={(value) => {
					const result = results.get(stackId)!;
					results.set(stackId, { ...result, approach: { type: value } });
				}}
				options={integrationOptions(stackStatus)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={highlighted} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}
	</IntegrationSeriesRow>
{/snippet}

<Modal
	testId={TestId.IntegrateUpstreamCommitsModal}
	bind:this={modal}
	{onClose}
	width={520}
	noPadding
	onSubmit={() => integrate()}
>
	<ScrollableContainer maxHeight="70vh">
		{#if base}
			<div class="section">
				<button
					type="button"
					class="section-toggle"
					class:expanded={incomingChangesExpanded}
					aria-expanded={incomingChangesExpanded}
					onclick={() => (incomingChangesExpanded = !incomingChangesExpanded)}
				>
					<span class="section-toggle-copy">
						<span class="section-toggle-icon"><Icon name="chevron-right" /></span>
						<span class="text-14 text-semibold">
							Incoming {base.upstreamCommits.length === 1 ? "change" : "changes"}
						</span>
						<Badge>{base.upstreamCommits.length}</Badge>
					</span>
				</button>
				{#if incomingChangesExpanded}
					<div class="scroll-wrap">
						<ScrollableContainer maxHeight="18rem">
							{#each base.upstreamCommits as commit}
								{@const commitUrl = forge.current.commitUrl(commit.id)}
								{@const selected = selectedIncomingCommitId === commit.id}
								<div class="incoming-change" class:selected>
									<button
										type="button"
										class="incoming-change-main"
										aria-expanded={selected}
										onclick={() => (selectedIncomingCommitId = selected ? undefined : commit.id)}
									>
										<Icon name={selected ? "chevron-down" : "chevron-right"} />
										<div class="incoming-change-copy">
											<span class="incoming-change-title text-13 text-semibold">
												{descriptionTitle(commit) ?? commit.id}
											</span>
											<span class="incoming-change-meta text-11">
												{shortSha(commit.id)} • {commit.author.name}
											</span>
										</div>
									</button>
									<div class="incoming-change-actions text-11">
										<button
											type="button"
											class="text-btn"
											onclick={() =>
												clipboardService.write(commit.id, { message: "Commit hash copied" })}
										>
											Copy SHA
										</button>
										{#if commitUrl}
											<button
												type="button"
												class="text-btn"
												onclick={() => urlService.openExternalUrl(commitUrl)}
											>
												Open
											</button>
										{/if}
									</div>
									{#if selected}
										{@const changesQuery = stackService.commitChanges(projectId, commit.id)}
										<div class="incoming-change-details">
											{#if changesQuery.response}
												{@const stats = changesQuery.response.stats}
												<div class="incoming-change-stats text-12">
													<span>{changesQuery.response.changes.length} touched files</span>
													{#if stats}
														<span>+{stats.linesAdded}</span>
														<span>-{stats.linesRemoved}</span>
													{/if}
												</div>
												<div class="incoming-change-files">
													{#each changesQuery.response.changes as change}
														<div class="incoming-change-file text-12">
															<span class="change-status"
																>{changeStatusLabel(change.status.type)}</span
															>
															<span class="change-path" title={change.path}>{change.path}</span>
														</div>
													{/each}
												</div>
											{:else}
												<p class="text-12 clr-text-2">Loading touched files…</p>
											{/if}
										</div>
									{/if}
								</div>
							{/each}
						</ScrollableContainer>
					</div>
				{/if}
			</div>
		{/if}
		<!-- CONFLICTED FILES -->
		{#if branchStatuses?.type === "updatesRequired" && branchStatuses?.worktreeConflicts.length > 0}
			<div class="section">
				<button
					type="button"
					class="section-toggle"
					class:expanded={conflictsExpanded}
					aria-expanded={conflictsExpanded}
					onclick={() => (conflictsExpanded = !conflictsExpanded)}
				>
					<span class="section-toggle-copy">
						<span class="section-toggle-icon"><Icon name="chevron-right" /></span>
						<span class="text-14 text-semibold">Conflicting uncommitted files</span>
						<Badge>{branchStatuses?.worktreeConflicts.length}</Badge>
					</span>
				</button>
				{#if conflictsExpanded}
					<p class="text-12 clr-text-2">
						These local files overlap with incoming changes. Updating will write conflict markers
						into them. Click a Unity file to inspect and resolve the conflict details.
					</p>
					<div class="scroll-wrap">
						<ScrollableContainer maxHeight="15rem">
							{@const conflicts = branchStatuses?.worktreeConflicts}
							{#each conflicts as file, i}
								{@const isUnityConflict = isUnityYamlPath(file)}
								<div class="conflict-row" class:is-last={i === conflicts.length - 1}>
									<FileListItem
										listMode="list"
										filePath={file}
										clickable={isUnityConflict}
										badges={isUnityConflict ? ["Unity"] : []}
										conflicted
										isLast
										onclick={isUnityConflict
											? () => {
													void unityConflictModal?.show(file);
												}
											: undefined}
									/>
									<div class="conflict-action">
										{#if isUnityConflict}
											<span class="text-11 clr-text-2">Click to inspect</span>
										{:else}
											<span class="text-11 clr-text-2">Conflict markers only</span>
										{/if}
									</div>
								</div>
							{/each}
						</ScrollableContainer>
					</div>
				{/if}
			</div>
		{/if}
		<!-- DIVERGED -->
		{#if base?.targetShaAheadOfRef}
			<div class="target-divergence">
				<img class="target-icon" src="/images/domain-icons/trunk.svg" alt="" />

				<div class="target-divergence-about">
					<h3 class="text-14 text-semibold">Target branch divergence</h3>
					<p class="text-12 text-body target-divergence-description">
						<span class="text-bold">target/main</span> has diverged from the workspace.
						<br />
						Select an action to proceed with updating.
					</p>
				</div>

				<div class="target-divergence-action">
					<Select
						value={baseResolutionApproach?.type}
						placeholder="Choose…"
						onselect={handleBaseResolutionSelection}
						options={baseResolutionOptions}
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={highlighted} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>
				</div>
			</div>
		{/if}
		<!-- STACKS AND BRANCHES TO UPDATE -->
		{#if statuses.length > 0}
			<div class="section" class:section-disabled={isDivergedResolved}>
				<h3 class="text-14 text-semibold">To be updated:</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="15rem">
						{#each statuses as { stack, status }}
							{#if stack.id}
								{@render stackStatus(stack.id, status)}
							{/if}
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
	</ScrollableContainer>

	{#if progressVisible}
		<div class="progress-tip">
			<div class="progress-tip-header">
				<div class="progress-tip-copy">
					<h3 class="text-14 text-semibold">
						{gitOperationProgress?.phaseLabel ?? "Updating workspace"}
					</h3>
					<p class="text-12 clr-text-2">
						{#if workspaceUpdateProgress}
							File {workspaceUpdateProgress.currentFile} of {workspaceUpdateProgress.totalFiles}
						{:else if gitOperationProgress?.detail}
							{gitOperationProgress.detail}
						{:else if gitOperationProgress}
							Elapsed {formatElapsed(currentElapsedMs(gitOperationProgress))}
						{:else}
							Preparing workspace update.
						{/if}
					</p>
				</div>

				{#if activeProgress !== undefined}
					<Badge>{formatProgressPercent(activeProgress)}</Badge>
				{:else if gitOperationProgress}
					<Badge>{formatElapsed(currentElapsedMs(gitOperationProgress))}</Badge>
				{/if}
			</div>

			{#if workspaceUpdateProgress || activeProgress !== undefined}
				<div class="progress-track" aria-hidden="true">
					<div class="progress-fill" style={`width: ${progressWidth(activeProgress)}%`}></div>
				</div>
			{/if}

			{#if workspaceUpdateProgress}
				<div class="progress-tip-meta text-12">
					<span>
						{formatFileSize(workspaceUpdateProgress.fileDownloadedBytes)} of
						{formatFileSize(workspaceUpdateProgress.fileTotalBytes)}
					</span>

					{#if workspaceUpdateProgress.bytesPerSecond !== undefined}
						<span>{formatTransferSpeed(workspaceUpdateProgress.bytesPerSecond)}</span>
					{/if}
				</div>

				<div class="progress-tip-path text-12" title={workspaceUpdateProgress.path}>
					{workspaceUpdateProgress.path}
				</div>
			{:else if gitOperationProgress?.path}
				<div class="progress-tip-path text-12" title={gitOperationProgress.path}>
					{gitOperationProgress.path}
				</div>
			{/if}
		</div>
	{/if}

	{#snippet controls()}
		<div class="controls-wrap">
			{#if progressVisible}
				<p class="controls-status text-12 clr-text-2">
					{workspaceUpdateStatusText(workspaceUpdateProgress, gitOperationProgress)}
				</p>
			{/if}

			<div class="controls">
				<Button onclick={() => modal?.close()} kind="outline">Cancel</Button>
				<AsyncButton
					testId={TestId.IntegrateUpstreamActionButton}
					wide
					style="pop"
					disabled={isDivergedResolved || !branchStatuses}
					loading={integratingUpstream === "loading" || !branchStatuses}
					tooltip={integratingUpstream === "loading"
						? workspaceUpdateTooltip(workspaceUpdateProgress, gitOperationProgress)
						: undefined}
					tooltipDelay={0}
					action={async () => {
						await integrate();
					}}
				>
					{integratingUpstream === "loading" ? "Updating workspace…" : "Update workspace"}
				</AsyncButton>
			</div>
		</div>
	{/snippet}
</Modal>

<UnityConflictResolverModal
	bind:this={unityConflictModal}
	{projectId}
	onResolved={() => {
		modal?.close();
	}}
/>

<style>
	/* INCOMING CHANGES */
	.section {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 14px;
		border-bottom: 1px solid var(--border-2);

		&:last-child {
			border-bottom: none;
		}

		.scroll-wrap {
			overflow: hidden;
			border: 1px solid var(--border-2);
			border-radius: var(--radius-m);
		}
	}

	.section-toggle {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		text-align: left;
	}

	.section-toggle-copy {
		display: flex;
		align-items: center;
		min-width: 0;
		gap: 6px;
	}

	.section-toggle-icon {
		display: flex;
		color: var(--text-3);
		transition: transform var(--transition-medium);
	}

	.section-toggle.expanded .section-toggle-icon {
		transform: rotate(90deg);
	}

	.incoming-change {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-1);

		&:last-child {
			border-bottom: none;
		}

		&.selected {
			background-color: var(--focus-bg-mute);
		}
	}

	.incoming-change-main {
		display: flex;
		align-items: center;
		min-width: 0;
		padding: 12px 8px 12px 12px;
		gap: 8px;
		text-align: left;
	}

	.incoming-change-copy {
		display: flex;
		flex-direction: column;
		min-width: 0;
		gap: 5px;
	}

	.incoming-change-title,
	.incoming-change-meta,
	.change-path {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.incoming-change-meta {
		color: var(--text-2);
	}

	.incoming-change-actions {
		display: flex;
		align-items: center;
		padding-right: 12px;
		gap: 8px;
	}

	.text-btn {
		color: var(--text-2);
		text-decoration: underline;
		text-underline-offset: 3px;

		&:hover {
			color: var(--text-1);
		}
	}

	.incoming-change-details {
		display: flex;
		grid-column: 1 / -1;
		flex-direction: column;
		padding: 0 12px 12px 34px;
		gap: 8px;
	}

	.incoming-change-stats {
		display: flex;
		gap: 10px;
		color: var(--text-2);
	}

	.incoming-change-files {
		display: flex;
		flex-direction: column;
		max-height: 9rem;
		overflow: auto;
		border: 1px solid var(--border-3);
		border-radius: var(--radius-s);
	}

	.incoming-change-file {
		display: grid;
		grid-template-columns: 72px minmax(0, 1fr);
		padding: 6px 8px;
		gap: 8px;
		border-bottom: 1px solid var(--border-3);

		&:last-child {
			border-bottom: none;
		}
	}

	.change-status {
		color: var(--text-2);
	}

	.conflict-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		border-bottom: 1px solid var(--border-3);
		background-color: var(--bg-danger);

		&.is-last {
			border-bottom: none;
		}
	}

	.conflict-action {
		display: flex;
		align-items: center;
		padding: 0 10px;
		gap: 8px;
	}

	/* DIVERGANCE */
	.target-divergence {
		display: flex;
		padding: 16px;
		gap: 14px;
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-warn);
	}

	.target-icon {
		width: 16px;
		height: 16px;
		border-radius: var(--radius-s);
	}

	.target-divergence-about {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 8px;
	}

	.target-divergence-description {
		color: var(--text-2);
	}

	.target-divergence-action {
		display: flex;
		flex-direction: column;
		max-width: 230px;
	}

	.progress-tip {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 12px;
		border-top: 1px solid var(--border-2);
		background: linear-gradient(
			180deg,
			color-mix(in srgb, var(--bg-1) 88%, var(--clr-theme-pop-element) 12%),
			var(--bg-1)
		);
	}

	.progress-tip-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
	}

	.progress-tip-copy {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.progress-track {
		height: 8px;
		overflow: hidden;
		border: 1px solid color-mix(in srgb, var(--border-2) 78%, transparent);
		border-radius: 999px;
		background-color: color-mix(in srgb, var(--bg-2) 88%, transparent);
	}

	.progress-fill {
		height: 100%;
		border-radius: inherit;
		background: linear-gradient(
			90deg,
			color-mix(in srgb, var(--clr-theme-pop-element) 82%, white 18%),
			var(--clr-theme-pop-element)
		);
		transition: width 160ms ease-out;
	}

	.progress-tip-meta {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		color: var(--text-2);
	}

	.progress-tip-path {
		overflow: hidden;
		color: var(--text-1);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* CONTROLS */
	.controls-wrap {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 8px;
	}

	.controls-status {
		padding: 0 16px;
	}

	.controls {
		display: flex;
		width: 100%;
		padding: 0 16px 16px;
		gap: 6px;
	}

	/* MODIFIERS */
	.section-disabled {
		opacity: 0.5;
		pointer-events: none;
	}
</style>
