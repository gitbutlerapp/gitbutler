<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import StackingAddSeriesModal from './StackingAddSeriesModal.svelte';
	import StackingStatusIcon from './StackingStatusIcon.svelte';
	import { getColorFromBranchType } from './stackingUtils';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import StackingSeriesHeaderContextMenu from '$lib/branch/StackingSeriesHeaderContextMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { stackingFeatureMultipleSeries } from '$lib/config/uiFeatureFlags';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import { showError } from '$lib/notifications/toasts';
	import PrDetailsModal from '$lib/pr/PrDetailsModal.svelte';
	import StackingPullRequestCard from '$lib/pr/StackingPullRequestCard.svelte';
	import { isFailure } from '$lib/result';
	import PopoverActionsContainer from '$lib/shared/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '$lib/shared/popoverActions/PopoverActionsItem.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { PatchSeries, VirtualBranch, type CommitStatus } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { slugify } from '@gitbutler/ui/utils/string';

	interface Props {
		currentSeries: PatchSeries;
		isTopSeries: boolean;
	}

	const { currentSeries, isTopSeries }: Props = $props();

	let descriptionVisible = $state(false);

	const project = getContext(Project);
	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);
	const branchStore = getContextStore(VirtualBranch);

	const aiGenEnabled = projectAiGenEnabled(project.id);
	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const prService = getGitHostPrService();
	const gitHost = getGitHost();

	const upstreamName = $derived(currentSeries.upstreamReference ? currentSeries.name : undefined);
	const gitHostBranch = $derived(upstreamName ? $gitHost?.branch(upstreamName) : undefined);
	const branch = $derived($branchStore);

	let stackingAddSeriesModal = $state<ReturnType<typeof StackingAddSeriesModal>>();
	let prDetailsModal = $state<ReturnType<typeof PrDetailsModal>>();
	let kebabContextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabContextMenuTrigger = $state<HTMLButtonElement>();

	let contextMenuOpened = $state(false);

	const topPatch = $derived(currentSeries?.patches[0]);
	const branchType = $derived<CommitStatus>(topPatch?.status ?? 'local');
	const lineColor = $derived(getColorFromBranchType(branchType));
	const hasNoCommits = $derived(
		currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0
	);

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getGitHostListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(listedPr?.number);

	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	const pr = $derived(prMonitor?.pr);
	const checksMonitor = $derived(
		$pr?.sourceBranch ? $gitHost?.checksMonitor($pr.sourceBranch) : undefined
	);

	async function handleReloadPR() {
		await Promise.allSettled([prMonitor?.refresh(), checksMonitor?.update()]);
	}

	function handleOpenPR(pushBeforeCreate: boolean = false) {
		prDetailsModal?.show(pushBeforeCreate);
	}

	function editTitle(title: string) {
		if (currentSeries?.name && title !== currentSeries.name) {
			branchController.updateSeriesName(branch.id, currentSeries.name, slugify(title));
		}
	}

	function editDescription(_description: string) {
		// branchController.updateBranchDescription(branch.id, description);
	}

	function addDescription() {
		descriptionVisible = true;
	}

	async function generateBranchName() {
		if (!aiGenEnabled || !currentSeries) return;

		const hunks = currentSeries.patches.flatMap((p) => p.files.flatMap((f) => f.hunks));

		const prompt = promptService.selectedBranchPrompt(project.id);
		const messageResult = await aiService.summarizeBranch({
			hunks,
			branchTemplate: prompt
		});

		if (isFailure(messageResult)) {
			console.error(messageResult.failure);
			showError('Failed to generate branch name', messageResult.failure);

			return;
		}

		const message = messageResult.value;

		if (message && message !== currentSeries.name) {
			branchController.updateSeriesName(branch.id, currentSeries.name, slugify(message));
		}
	}
</script>

<StackingAddSeriesModal bind:this={stackingAddSeriesModal} parentSeriesName={currentSeries.name} />

<StackingSeriesHeaderContextMenu
	bind:contextMenuEl={kebabContextMenu}
	target={kebabContextMenuTrigger}
	headName={currentSeries.name}
	seriesCount={branch.series?.length ?? 0}
	{addDescription}
	onGenerateBranchName={generateBranchName}
	hasGitHostBranch={!!gitHostBranch}
	hasPr={!!$pr}
	openPrDetailsModal={handleOpenPR}
	reloadPR={handleReloadPR}
	onopen={() => (contextMenuOpened = true)}
	onclose={() => (contextMenuOpened = false)}
/>

<div role="article" class="branch-header" oncontextmenu={(e) => e.preventDefault()}>
	<PopoverActionsContainer class="branch-actions-menu" stayOpen={contextMenuOpened}>
		{#if $stackingFeatureMultipleSeries}
			<PopoverActionsItem
				icon="plus-small"
				tooltip="Add dependent branch"
				onclick={() => {
					stackingAddSeriesModal?.show();
				}}
			/>
		{/if}
		{#if gitHostBranch}
			<PopoverActionsItem
				icon="open-link"
				tooltip="Open in browser"
				onclick={() => {
					const url = gitHostBranch?.url;
					if (url) openExternalUrl(url);
				}}
			/>
		{/if}
		<PopoverActionsItem
			bind:el={kebabContextMenuTrigger}
			activated={contextMenuOpened}
			icon="kebab"
			tooltip="More options"
			onclick={() => {
				kebabContextMenu?.toggle();
			}}
		/>
	</PopoverActionsContainer>

	<div class="branch-info">
		<StackingStatusIcon
			lineTop={isTopSeries ? false : true}
			icon={branchType === 'integrated' ? 'tick-small' : 'remote-branch-small'}
			iconColor="var(--clr-core-ntrl-100)"
			color={lineColor}
			lineBottom={currentSeries.patches.length > 0}
		/>
		<div class="text-14 text-bold branch-info__name">
			<span class:no-upstream={!gitHostBranch} class="remote-name">
				{$baseBranch.remoteName ? `${$baseBranch.remoteName} /` : 'origin /'}
			</span>
			<BranchLabel
				name={currentSeries.name}
				onChange={(name) => editTitle(name)}
				disabled={!!gitHostBranch}
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
	{#if $prService && !hasNoCommits}
		<div class="branch-action">
			<div class="branch-action__line" style:--bg-color={lineColor}></div>
			<div class="branch-action__body">
				{#if $pr}
					<StackingPullRequestCard upstreamName={currentSeries.name} />
				{:else}
					<Button
						style="ghost"
						wide
						outline
						disabled={currentSeries.patches.length === 0 || !$gitHost || !$prService}
						onclick={() => handleOpenPR(!gitHostBranch)}
					>
						Create pull request
					</Button>
				{/if}
			</div>
		</div>
	{/if}

	{#if $pr}
		<PrDetailsModal bind:this={prDetailsModal} type="display" pr={$pr} />
	{:else}
		<PrDetailsModal bind:this={prDetailsModal} type="preview-series" {currentSeries} />
	{/if}
</div>

<style lang="postcss">
	.branch-header {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&:hover,
		&:focus-within {
			& :global(.branch-actions-menu) {
				--show: true;
			}
		}
	}

	.branch-info {
		width: 100%;
		padding-right: 12px;
		display: flex;
		justify-content: flex-start;
		align-items: center;

		& .branch-info__name {
			display: flex;
			align-items: center;
			justify-content: flex-start;
			min-width: 0;
			flex-grow: 1;
		}

		.remote-name {
			min-width: max-content;
			color: var(--clr-scale-ntrl-60);

			&.no-upstream {
				/**
				 * Element is requird to still be there, so we can use
				 * it to wiggle 5px to the left to align the BranchLabel
				 * Input/Label component.
				 */
				visibility: hidden;
				max-width: 0px;
				max-height: 0px;
				margin-right: -5px;
			}
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
			padding: 0 14px 14px 0;
		}
	}

	.branch-action__line {
		min-width: 2px;
		margin: 0 20px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>
