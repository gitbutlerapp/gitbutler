<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { descriptionTitle } from "$lib/commits/commit";
	import { parseQueryError } from "$lib/error/error";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
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
		SimpleCommitRow,
		FileListItem,
		Select,
		SelectItem,
		ScrollableContainer,
		type BranchShouldBeDeletedMap,
		TestId,
		AsyncButton,
	} from "@gitbutler/ui";
	import { tick } from "svelte";
	import { SvelteMap } from "svelte/reactivity";
	import type {
		BaseBranchResolutionApproach,
		BottomUpdate,
		BottomUpdateKind,
		BranchStatus,
		StackStatus,
		StackStatuses,
	} from "@gitbutler/but-sdk";

	type ResolutionKind = BottomUpdateKind | "skip";

	function stackFullyIntegrated(stackStatus: StackStatus): boolean {
		return (
			stackStatus.branchStatuses.every((b) => b.status.type === "integrated") &&
			stackStatus.treeStatus.type === "empty"
		);
	}

	interface Props {
		projectId: string;
		onClose?: () => void;
	}

	const { projectId, onClose }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchQuery.response);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	let modal = $state<Modal>();
	let integratingUpstream = $state(false);
	let integrationError = $state<string | undefined>();
	// Maps stackId → chosen approach (rebase, merge, or skip)
	const resolutionKinds = new SvelteMap<string, ResolutionKind>();
	const baseResolutionOptions = [
		{ label: "Rebase", value: "rebase" as const },
		{ label: "Merge", value: "merge" as const },
		{ label: "Hard reset", value: "hardReset" as const },
	];
	let baseResolutionApproach = $state<BaseBranchResolutionApproach | undefined>();
	let targetCommitOid = $state<string | undefined>(undefined);

	const isDivergedResolved = $derived(base?.targetShaAheadOfRef && !baseResolutionApproach);

	// Reactive subscription — only active while the modal is open, auto-refetches when
	// invalidated (e.g. by a background fetch updating the remote ref).
	const statuses = $derived(
		modal?.imports.open
			? stackService.upstreamStatusesQuery(projectId, targetCommitOid)
			: undefined,
	);

	function stackName(status: StackStatus): string {
		return status.branchStatuses.at(-1)?.name ?? "Unknown";
	}

	/** Extract and sort status entries from the API response. */
	function getStatusEntries(response: StackStatuses): StackStatus[] {
		if (response.type === "upToDate") return [];
		const entries = response.subject.statuses.filter(
			(s): s is StackStatus & { stackId: string } => s.stackId !== null,
		);
		// Sort: non-integrated stacks first (alphabetically), then fully-integrated ones
		entries.sort((a, b) => {
			const aIntegrated = stackFullyIntegrated(a);
			const bIntegrated = stackFullyIntegrated(b);
			if (aIntegrated !== bIntegrated) return aIntegrated ? 1 : -1;
			return stackName(a).localeCompare(stackName(b));
		});
		return entries;
	}

	// Refresh resolution kinds when statuses change
	$effect(() => {
		const response = statuses?.response;
		resolutionKinds.clear();
		if (!response) return;
		for (const status of getStatusEntries(response)) {
			if (!stackFullyIntegrated(status)) {
				resolutionKinds.set(status.stackId!, "rebase");
			}
		}
	});

	// Resolve the target commit oid if the base branch diverged and the resolution
	// approach is changed
	$effect(() => {
		if (!modal?.imports.open) return;
		if (base?.targetShaAheadOfRef && baseResolutionApproach) {
			stackService
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

	function someBranchesShouldNotBeDeleted(branchNames: string[]): boolean {
		return branchNames.some((name) => getBooleanStorageItem(getDontDeleteBranchStorageKey(name)));
	}

	/** Build BottomUpdate[] from current statuses and chosen resolution kinds. */
	function buildUpdates(entries: StackStatus[]): BottomUpdate[] {
		const updates: BottomUpdate[] = [];
		for (const status of entries) {
			if (!status.stackId || stackFullyIntegrated(status)) continue;
			const kind = resolutionKinds.get(status.stackId) ?? "rebase";
			if (kind === "skip") continue;
			const selector = status.bottomSelector;
			if (!selector) continue;
			updates.push({ kind, selector });
		}
		return updates;
	}

	function getDontDeleteBranchStorageKey(branchName: string): string {
		return `integrate-upstream-modal:dont-delete-branch:${projectId}:${branchName}`;
	}

	function handleBaseResolutionSelection(value: BaseBranchResolutionApproach["type"]) {
		baseResolutionApproach = { type: value };
	}

	async function integrate() {
		integratingUpstream = true;
		integrationError = undefined;

		await tick();

		try {
			// Resolve base branch divergence first if needed
			if (base?.diverged && baseResolutionApproach) {
				await stackService.resolveUpstreamIntegrationMutation({
					projectId,
					resolutionApproach: baseResolutionApproach,
				});
			}

			// Compute current statuses for the integration
			const response = statuses?.response;
			const entries = response ? getStatusEntries(response) : [];

			// Unapply fully-integrated stacks
			for (const status of entries) {
				if (status.stackId && stackFullyIntegrated(status)) {
					await stackService.unapply({ projectId, stackId: status.stackId });
				}
			}

			// Rebase/merge remaining stacks via the new API
			const updates = buildUpdates(entries);
			if (updates.length > 0) {
				await stackService.workspaceIntegrateUpstream({
					projectId,
					updates,
					dryRun: false,
				});
			}

			await baseBranchService.refreshBaseBranch(projectId);
			integratingUpstream = false;
			modal?.close();
		} catch (err: unknown) {
			integrationError = parseQueryError(err).message;
			integratingUpstream = false;
		}
	}

	export async function show() {
		integratingUpstream = false;
		integrationError = undefined;
		await tick();
		modal?.show();
		// Statuses load automatically via the reactive query subscription
		// when the modal opens (gated on modal?.imports.open).
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		},
	};

	function branchStatusToRowEntry(
		branchStatus: BranchStatus,
	): "integrated" | "conflicted" | "clear" {
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
		const statuses = stackStatus.branchStatuses.map((series) => ({
			name: series.name,
			status: branchStatusToRowEntry(series.status),
		}));

		statuses.reverse();

		return statuses;
	}
	function getBranchShouldBeDeletedMap(
		stackId: string,
		stackStatus: StackStatus,
	): BranchShouldBeDeletedMap {
		const branchShouldBeDeletedMap: BranchShouldBeDeletedMap = {};
		const dontDelete = someBranchesShouldNotBeDeleted(
			stackStatus.branchStatuses.map((b) => b.name),
		);
		stackStatus.branchStatuses.forEach((branch) => {
			branchShouldBeDeletedMap[branch.name] = !dontDelete;
		});
		return branchShouldBeDeletedMap;
	}

	function updateBranchShouldBeDeletedMap(
		_stackId: string,
		branchNames: string[],
		shouldBeDeleted: boolean,
	): void {
		for (const branchName of branchNames) {
			const key = getDontDeleteBranchStorageKey(branchName);
			if (!shouldBeDeleted) {
				setBooleanStorageItem(key, true);
			} else {
				removeStorageItem(key);
			}
		}
	}

	function integrationOptions(
		stackStatus: StackStatus,
	): { label: string; value: ResolutionKind }[] {
		const options: { label: string; value: ResolutionKind }[] = [
			{ label: "Rebase", value: "rebase" },
		];
		if (stackStatus.branchStatuses.length <= 1) {
			options.push({ label: "Merge", value: "merge" });
		}
		options.push({ label: "Leave as is", value: "skip" });
		return options;
	}
</script>

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
				<h3 class="text-14 text-semibold section-title">
					<span>Incoming {base.upstreamCommits.length === 1 ? "change" : "changes"}</span><Badge
						>{base.upstreamCommits.length}</Badge
					>
				</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="16.5rem">
						{#each base.upstreamCommits as commit}
							{@const commitUrl = forge.current.commitUrl(commit.id)}
							<SimpleCommitRow
								title={descriptionTitle(commit) ?? ""}
								sha={commit.id}
								date={new Date(commit.createdAt)}
								author={commit.author.name}
								url={commitUrl}
								onOpen={(url) => urlService.openExternalUrl(url)}
								onCopy={() => clipboardService.write(commit.id, { message: "Commit hash copied" })}
							/>
						{/each}
					</ScrollableContainer>
				</div>
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
		<ReduxResult {projectId} result={statuses?.result} hideError>
			{#snippet children(response)}
				{@const entries = getStatusEntries(response)}
				<!-- CONFLICTED FILES -->
				{#if response.type === "updatesRequired" && response.subject.worktreeConflicts.length > 0}
					{@const conflicts = response.subject.worktreeConflicts}
					<div class="section">
						<h3 class="text-14 text-semibold section-title">
							<span>Conflicting uncommitted files</span>

							<Badge>{conflicts.length}</Badge>
						</h3>
						<p class="text-12 clr-text-2">
							Updating the workspace will add conflict markers to the following files.
						</p>
						<div class="scroll-wrap">
							<ScrollableContainer maxHeight="15rem">
								{#each conflicts as file, i}
									<FileListItem
										listMode="list"
										filePath={file}
										clickable={false}
										conflicted
										isLast={i === conflicts.length - 1}
									/>
								{/each}
							</ScrollableContainer>
						</div>
					</div>
				{/if}
				<!-- STACKS TO UPDATE -->
				{#if entries.length > 0}
					<div class="section" class:section-disabled={isDivergedResolved}>
						<h3 class="text-14 text-semibold">To be updated:</h3>
						<div class="scroll-wrap">
							<ScrollableContainer maxHeight="15rem">
								{#each entries as status}
									{@const stackId = status.stackId!}
									{@const branchShouldBeDeletedMap = getBranchShouldBeDeletedMap(stackId, status)}
									<IntegrationSeriesRow
										testId={TestId.IntegrateUpstreamSeriesRow}
										series={integrationRowSeries(status)}
										{branchShouldBeDeletedMap}
										updateBranchShouldBeDeletedMap={(branchNames, shouldBeDeleted) =>
											updateBranchShouldBeDeletedMap(stackId, branchNames, shouldBeDeleted)}
									>
										{#if !stackFullyIntegrated(status) && resolutionKinds.has(stackId)}
											<Select
												value={resolutionKinds.get(stackId)}
												maxWidth={130}
												onselect={(value) => {
													resolutionKinds.set(stackId, value);
												}}
												options={integrationOptions(status)}
											>
												{#snippet itemSnippet({ item, highlighted })}
													<SelectItem selected={highlighted} {highlighted}>
														{item.label}
													</SelectItem>
												{/snippet}
											</Select>
										{/if}
									</IntegrationSeriesRow>
								{/each}
							</ScrollableContainer>
						</div>
					</div>
				{/if}
			{/snippet}
		</ReduxResult>
		{#if integrationError}
			<div class="section">
				<p class="text-13 text-body text-error" style="user-select: text">{integrationError}</p>
			</div>
		{/if}
	</ScrollableContainer>

	{#snippet controls()}
		<div class="controls">
			<Button onclick={() => modal?.close()} kind="outline">Cancel</Button>
			<AsyncButton
				testId={TestId.IntegrateUpstreamActionButton}
				wide
				style="pop"
				disabled={isDivergedResolved || !statuses?.response}
				loading={integratingUpstream || !statuses?.response}
				action={async () => {
					await integrate();
				}}
			>
				Update workspace
			</AsyncButton>
		</div>
	{/snippet}
</Modal>

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

	.section-title {
		display: flex;
		align-items: center;
		gap: 4px;
	}

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

	/* CONTROLS */
	.controls {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	/* MODIFIERS */
	.section-disabled {
		opacity: 0.5;
		pointer-events: none;
	}

	.text-error {
		color: var(--text-error);
		word-break: break-word;
	}
</style>
