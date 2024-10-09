<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import StackingStatusIcon from './StackingStatusIcon.svelte';
	import { getColorFromBranchType } from './stackingUtils';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import StackingBranchHeaderContextMenu from '$lib/branch/StackingBranchHeaderContextMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import PrDetailsModal from '$lib/pr/PrDetailsModal.svelte';
	import StackingPullRequestCard from '$lib/pr/StackingPullRequestCard.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { DetailedCommit, VirtualBranch, type CommitStatus } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		name: string;
		upstreamName?: string;
		commits: DetailedCommit[];
	}

	const { name, upstreamName, commits }: Props = $props();

	let descriptionVisible = $state(false);

	const branchStore = getContextStore(VirtualBranch);
	const branch = $derived($branchStore);

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const prService = getGitHostPrService();
	const gitHost = getGitHost();
	const gitHostBranch = $derived(upstreamName ? $gitHost?.branch(upstreamName) : undefined);

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let prDetailsModal = $state<ReturnType<typeof PrDetailsModal>>();
	let meatballButtonEl = $state<HTMLDivElement>();

	const currentSeries = $derived(branch.series?.find((series) => series.name === upstreamName));
	const topPatch = $derived(currentSeries?.patches[0]);
	const hasShadow = $derived.by(() => {
		if (!topPatch || !topPatch.remoteCommitId) return false;

		if (topPatch.remoteCommitId !== topPatch.id) return true;

		return false;
	});
	const branchColorType = $derived<CommitStatus | 'localAndShadow'>(
		hasShadow ? 'localAndShadow' : (topPatch?.status ?? 'local')
	);
	const lineColor = $derived(getColorFromBranchType(branchColorType));

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getGitHostListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(listedPr?.number);

	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	const pr = $derived(prMonitor?.pr);

	function handleOpenPR() {
		prDetailsModal?.show();
	}

	function editTitle(title: string) {
		branchController.updateBranchName(branch.id, title);
	}

	function editDescription(_description: string) {
		// branchController.updateBranchDescription(branch.id, description);
	}

	function addDescription() {
		descriptionVisible = true;
	}
</script>

<div class="branch-header">
	<div class="branch-info">
		<StackingStatusIcon icon="tick-small" iconColor="#fff" color={lineColor} gap={false} lineTop />
		<div class="text-14 text-bold branch-info__name">
			<span class="remote-name">{$baseBranch.remoteName ?? 'origin'}/</span>
			<BranchLabel {name} onChange={(name) => editTitle(name)} />
			<Button
				size="tag"
				icon="open-link"
				style="ghost"
				onclick={(e: MouseEvent) => {
					const url = gitHostBranch?.url;
					if (url) openExternalUrl(url);
					e.preventDefault();
					e.stopPropagation();
				}}
			></Button>
		</div>
		<div class="branch-info__btns">
			<Button
				size="tag"
				icon="kebab"
				style="ghost"
				bind:el={meatballButtonEl}
				onclick={() => {
					contextMenu?.toggle();
				}}
			></Button>
			<StackingBranchHeaderContextMenu
				bind:contextMenuEl={contextMenu}
				target={meatballButtonEl}
				headName={name}
				seriesCount={branch.series?.length ?? 0}
				{addDescription}
			/>
		</div>
	</div>
	{#if descriptionVisible}
		<div class="branch-info__description">
			<div class="branch-action__line" style:--bg-color={lineColor}></div>
			<BranchLabel
				name={branch.description}
				onChange={(description) => editDescription(description)}
			/>
		</div>
	{/if}
	<div class="branch-action">
		<div class="branch-action__line" style:--bg-color={lineColor}></div>
		<div class="branch-action__body">
			{#if $pr}
				<StackingPullRequestCard pr={$pr} {prMonitor} sourceBranch={$pr.sourceBranch} />
			{:else}
				<Button
					style="ghost"
					wide
					outline
					disabled={commits.length === 0 || !$gitHost || !$prService}
					onclick={handleOpenPR}>Create pull request</Button
				>
			{/if}
		</div>
	</div>
</div>

<PrDetailsModal bind:this={prDetailsModal} type="preview-series" {upstreamName} {name} {commits} />

<style lang="postcss">
	.branch-header {
		display: flex;
		border-bottom: 1px solid var(--clr-border-2);
		display: flex;
		flex-direction: column;
	}

	.branch-info {
		padding: 0 13px;
		display: flex;
		justify-content: flex-start;
		align-items: center;

		& .branch-info__name {
			display: flex;
			align-items: stretch;
			justify-content: start;
			padding: 8px 16px;
			min-width: 0;
			flex-grow: 1;
		}

		& .branch-info__btns {
			display: flex;
			gap: 0.25rem;
		}

		.remote-name {
			margin-top: 3px;
			color: var(--clr-scale-ntrl-60);
		}
	}

	.branch-info__description {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;
	}

	.branch-action {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;

		.branch-action__body {
			width: 100%;
			padding: 4px 12px 12px 0px;
		}
	}

	.branch-action__line {
		margin: 0 22px 0 22.5px;
		border-left: 2px solid var(--bg-color, var(--clr-border-3));
	}
</style>
