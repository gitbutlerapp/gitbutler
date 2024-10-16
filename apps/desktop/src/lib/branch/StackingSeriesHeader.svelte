<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import StackingStatusIcon from './StackingStatusIcon.svelte';
	import { getColorFromBranchType } from './stackingUtils';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import StackingSeriesHeaderContextMenu from '$lib/branch/StackingSeriesHeaderContextMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import { showError } from '$lib/notifications/toasts';
	import PrDetailsModal from '$lib/pr/PrDetailsModal.svelte';
	import StackingPullRequestCard from '$lib/pr/StackingPullRequestCard.svelte';
	import { isFailure } from '$lib/result';
	import { slugify } from '$lib/utils/string';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { PatchSeries, VirtualBranch, type CommitStatus } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';

	interface Props {
		currentSeries: PatchSeries;
	}

	const { currentSeries }: Props = $props();

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

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let prDetailsModal = $state<ReturnType<typeof PrDetailsModal>>();
	let meatballButtonEl = $state<HTMLDivElement>();

	// TODO: Simplify figuring out if shadow color is needed
	const topPatch = $derived(currentSeries?.patches[0]);
	const hasShadow = $derived.by(() => {
		if (!topPatch || !topPatch.remoteCommitId) return false;
		if (topPatch.remoteCommitId !== topPatch.id) return true;
		return false;
	});
	const branchType = $derived<CommitStatus | 'localAndShadow'>(
		hasShadow ? 'localAndShadow' : topPatch?.status ?? 'local'
	);
	const lineColor = $derived(getColorFromBranchType(branchType));

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

	function handleOpenPR() {
		prDetailsModal?.show();
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

<div class="branch-header">
	<div class="branch-info">
		<StackingStatusIcon
			icon={branchType === 'integrated' ? 'tick-small' : 'remote-branch-small'}
			iconColor="#fff"
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
			{#if gitHostBranch}
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
			{/if}
		</div>
		<div class="branch-info__btns">
			<Button
				icon="kebab"
				style="ghost"
				bind:el={meatballButtonEl}
				onclick={() => {
					contextMenu?.toggle();
				}}
			></Button>
			<StackingSeriesHeaderContextMenu
				bind:contextMenuEl={contextMenu}
				target={meatballButtonEl}
				headName={currentSeries.name}
				seriesCount={branch.series?.length ?? 0}
				{addDescription}
				onGenerateBranchName={generateBranchName}
				disableTitleEdit={!!gitHostBranch}
				hasPr={!!$pr}
				openPrDetailsModal={handleOpenPR}
				reloadPR={handleReloadPR}
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
	{#if gitHostBranch}
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
						disabled={currentSeries.patches.length === 0 || !$gitHost || !$prService}
						onclick={handleOpenPR}>Create pull request</Button
					>
				{/if}
			</div>
		</div>
	{/if}
	{#if currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0}
		<div class="branch-emptystate">
			<EmptyStatePlaceholder bottomMargin={10}>
				{#snippet title()}
					This is an empty series
				{/snippet}
				{#snippet caption()}
					All your commits will land here
				{/snippet}
			</EmptyStatePlaceholder>
		</div>
	{/if}

	{#if $pr}
		<PrDetailsModal bind:this={prDetailsModal} type="display" pr={$pr} />
	{:else}
		<PrDetailsModal
			bind:this={prDetailsModal}
			type="preview-series"
			{upstreamName}
			name={currentSeries.name}
			commits={currentSeries.patches}
		/>
	{/if}
</div>

<style lang="postcss">
	.branch-header {
		display: flex;
		display: flex;
		flex-direction: column;
		overflow: hidden;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.branch-info {
		padding-right: 13px;
		display: flex;
		justify-content: flex-start;
		align-items: center;

		& .branch-info__name {
			display: flex;
			align-items: stretch;
			justify-content: flex-start;
			min-width: 0;
			flex-grow: 1;
		}

		& .branch-info__btns {
			display: flex;
			gap: 0.25rem;
		}

		.remote-name {
			margin-top: 3px;
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
			padding: 0 12px 12px 0;
		}
	}

	.branch-action__line {
		min-width: 2px;
		margin: 0 22px;
		background-color: var(--bg-color, var(--clr-border-3));
	}

	.branch-emptystate {
		width: 100%;
		display: flex;
		justify-content: center;
		align-items: center;

		border-top: 2px solid var(--bg-color, var(--clr-border-3));
	}
</style>
