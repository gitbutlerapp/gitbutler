<script lang="ts">
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { descriptionTitle } from "$lib/commits/commit";
	import { commitUrl, FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import {
		sortUpstreamIntegrationStatus,
		type UpstreamIntegrationStackStatus,
		type UpstreamIntegrationStatuses,
	} from "$lib/upstream/types";
	import { UPSTREAM_INTEGRATION_SERVICE } from "$lib/upstream/upstreamIntegrationService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		Badge,
		Button,
		FileListItem,
		IntegrationSeriesRow,
		Modal,
		SimpleCommitRow,
		ScrollableContainer,
		TestId,
		AsyncButton,
	} from "@gitbutler/ui";
	import { tick } from "svelte";

	type OperationState = "inert" | "loading" | "completed";

	interface Props {
		projectId: string;
		onClose?: () => void;
	}

	const { projectId, onClose }: Props = $props();

	const upstreamIntegrationService = inject(UPSTREAM_INTEGRATION_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchQuery.response);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>("inert");
	let statuses = $state<UpstreamIntegrationStackStatus[]>([]);
	let integrationStatuses = $state<UpstreamIntegrationStatuses | undefined>();
	let statusRequest = 0;

	const baseLoaded = $derived(!!base);
	const isBaseDiverged = $derived(!!base?.targetShaAheadOfRef);
	const canIntegrate = $derived(
		baseLoaded &&
			!isBaseDiverged &&
			!!integrationStatuses &&
			integrationStatuses.updates.length > 0,
	);
	const worktreeConflicts = $derived(integrationStatuses?.worktreeConflicts ?? []);
	const [integrateUpstream] = $derived(upstreamIntegrationService.integrateUpstream());

	async function loadStatuses() {
		const request = ++statusRequest;
		integrationStatuses = undefined;
		statuses = [];

		const nextStatuses = await upstreamIntegrationService.upstreamStatuses(projectId);
		if (request !== statusRequest || isBaseDiverged) return;

		const statusesTmp = [...nextStatuses.subject];
		statusesTmp.sort(sortUpstreamIntegrationStatus);

		integrationStatuses = nextStatuses;
		statuses = statusesTmp;
	}

	$effect(() => {
		if (!modal?.imports.open) return;

		if (!baseLoaded) {
			statusRequest++;
			integrationStatuses = undefined;
			statuses = [];
			return;
		}

		if (isBaseDiverged) {
			statusRequest++;
			integrationStatuses = undefined;
			statuses = [];
			return;
		}

		void loadStatuses();
	});

	async function integrate() {
		if (!canIntegrate || !integrationStatuses) return;

		integratingUpstream = "loading";
		await tick();

		await integrateUpstream({
			projectId,
			updates: integrationStatuses.updates,
			dryRun: false,
		});
		await baseBranchService.refreshBaseBranch(projectId);
		integratingUpstream = "completed";
		modal?.close();
	}

	export async function show() {
		integratingUpstream = "inert";
		integrationStatuses = undefined;
		statuses = [];
		await tick();
		modal?.show();
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		},
	};

	function integrationRowSeries(
		statusInfo: UpstreamIntegrationStackStatus,
	): { name: string; status: "integrated" | "conflicted" | "clear" }[] {
		return [...statusInfo.branchStatuses].reverse();
	}
</script>

{#snippet stackStatus(statusInfo: UpstreamIntegrationStackStatus)}
	<IntegrationSeriesRow
		testId={TestId.IntegrateUpstreamSeriesRow}
		series={integrationRowSeries(statusInfo)}
		branchShouldBeDeletedMap={{}}
		updateBranchShouldBeDeletedMap={() => {}}
		showDeleteControls={false}
	/>
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
				<h3 class="text-14 text-semibold section-title">
					<span>Incoming {base.upstreamCommits.length === 1 ? "change" : "changes"}</span><Badge
						>{base.upstreamCommits.length}</Badge
					>
				</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="16.5rem">
						{#each base.upstreamCommits as commit}
							{@const url = forgeInfo ? commitUrl(forgeInfo, commit.id) : undefined}
							<SimpleCommitRow
								title={descriptionTitle(commit) ?? ""}
								sha={commit.id}
								date={new Date(commit.createdAt)}
								author={commit.author.name}
								{url}
								onOpen={(url) => urlService.openExternalUrl(url)}
								onCopy={() => clipboardService.write(commit.id, { message: "Commit hash copied" })}
							/>
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
		<!-- DIVERGED -->
		{#if isBaseDiverged}
			<div class="target-divergence">
				<img class="target-icon" src="/images/domain-icons/trunk.svg" alt="" />

				<div class="target-divergence-about">
					<h3 class="text-14 text-semibold">Target branch divergence</h3>
					<p class="text-12 text-body target-divergence-description">
						<span class="text-bold">{base?.branchName ?? "The target branch"}</span> has diverged
						from the workspace.
						<br />
						Resolve target branch divergence before updating the workspace.
					</p>
				</div>
			</div>
		{/if}
		<!-- STACKS AND BRANCHES TO UPDATE -->
		{#if statuses.length > 0}
			<div class="section">
				<h3 class="text-14 text-semibold">To be updated:</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="15rem">
						{#each statuses as status}
							{@render stackStatus(status)}
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
		{#if worktreeConflicts.length > 0}
			<div class="worktree-conflicts" data-testid={TestId.IntegrateUpstreamWorktreeConflicts}>
				<h3 class="text-14 text-semibold">Uncommitted changes conflict</h3>
				<p class="text-12 text-body worktree-conflicts-description">
					These files will conflict when your current uncommitted changes are applied onto the
					updated workspace.
					<br />
					You're free to proceed, but conflict markers will be added to your uncommitted work.
				</p>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="10rem">
						{#each worktreeConflicts as path, i}
							<FileListItem
								filePath={path}
								clickable={false}
								conflicted
								conflictHint="May conflict"
								isLast={i === worktreeConflicts.length - 1}
							/>
						{/each}
					</ScrollableContainer>
				</div>
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
				disabled={!canIntegrate}
				loading={integratingUpstream === "loading" || (!integrationStatuses && !isBaseDiverged)}
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

	.worktree-conflicts {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 10px;
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-warn);

		.scroll-wrap {
			overflow: hidden;
			border: 1px solid var(--border-2);
			border-radius: var(--radius-m);
			background-color: var(--bg-1);
		}
	}

	.worktree-conflicts-description {
		color: var(--text-2);
	}

	/* CONTROLS */
	.controls {
		display: flex;
		width: 100%;
		gap: 6px;
	}
</style>
