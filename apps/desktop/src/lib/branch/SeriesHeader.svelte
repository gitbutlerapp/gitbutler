<script lang="ts">
	import AddSeriesModal from './AddSeriesModal.svelte';
	import BranchLabel from './BranchLabel.svelte';
	import Dropzones from './Dropzones.svelte';
	import SeriesDescription from './SeriesDescription.svelte';
	import SeriesHeaderStatusIcon from './SeriesHeaderStatusIcon.svelte';
	import { getColorFromBranchType } from './stackingUtils';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { Project, ProjectService } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import SeriesHeaderContextMenu from '$lib/branch/SeriesHeaderContextMenu.svelte';
	import { CloudBranchCreationService } from '$lib/branch/cloudBranchCreationService';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { stackingFeatureMultipleSeries } from '$lib/config/uiFeatureFlags';
	import { getForge } from '$lib/forge/interface/forge';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { showError } from '$lib/notifications/toasts';
	import PrDetailsModal from '$lib/pr/PrDetailsModal.svelte';
	import PullRequestCard from '$lib/pr/PullRequestCard.svelte';
	import { isFailure } from '$lib/result';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { listCommitFiles } from '$lib/vbranches/remoteCommits';
	import { PatchSeries, VirtualBranch, type CommitStatus } from '$lib/vbranches/types';
	import { CloudBranchesService } from '@gitbutler/shared/cloud/stacks/service';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import { tick } from 'svelte';

	interface Props {
		currentSeries: PatchSeries;
		isTopSeries: boolean;
	}

	const { currentSeries, isTopSeries }: Props = $props();

	let descriptionVisible = $state(!!currentSeries.description);

	const project = getContext(Project);
	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);
	const branchStore = getContextStore(VirtualBranch);

	const aiGenEnabled = projectAiGenEnabled(project.id);
	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const prService = getForgePrService();
	const forge = getForge();

	const upstreamName = $derived(currentSeries.upstreamReference ? currentSeries.name : undefined);
	const forgeBranch = $derived(upstreamName ? $forge?.branch(upstreamName) : undefined);
	const branch = $derived($branchStore);

	let stackingAddSeriesModal = $state<ReturnType<typeof AddSeriesModal>>();
	let prDetailsModal = $state<ReturnType<typeof PrDetailsModal>>();
	let kebabContextMenu = $state<ReturnType<typeof ContextMenu>>();
	let stackingContextMenu = $state<ReturnType<typeof SeriesHeaderContextMenu>>();
	let kebabContextMenuTrigger = $state<HTMLButtonElement>();
	let seriesHeaderEl = $state<HTMLDivElement>();
	let seriesDescriptionEl = $state<HTMLTextAreaElement>();

	let contextMenuOpened = $state(false);

	const topPatch = $derived(currentSeries?.patches[0]);
	const branchType = $derived<CommitStatus>(topPatch?.status ?? 'local');
	const lineColor = $derived(getColorFromBranchType(branchType));
	const hasNoCommits = $derived(
		currentSeries.upstreamPatches.length === 0 && currentSeries.patches.length === 0
	);

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getForgeListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(currentSeries.prNumber || listedPr?.number);

	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	const pr = $derived(prMonitor?.pr);
	const checksMonitor = $derived(
		$pr?.sourceBranch ? $forge?.checksMonitor($pr.sourceBranch) : undefined
	);

	const projectService = getContext(ProjectService);
	const cloudEnabled = projectService.cloudEnabled;

	const cloudBranchCreationService = getContext(CloudBranchCreationService);
	const cloudBranchesService = getContext(CloudBranchesService);
	const cloudBranch = $derived(cloudBranchesService.branchForBranchId(branch.id));
	const showCreateCloudBranch = $derived(
		$cloudEnabled &&
			cloudBranchCreationService.canCreateBranch &&
			$cloudBranch.state === 'not-found'
	);

	/**
	 * We are starting to store pull request id's locally so if we find one that does not have
	 * one locally stored then we set it once.
	 *
	 * TODO: Remove this after transition is complete.
	 */
	$effect(() => {
		if (
			$forge?.name === 'github' &&
			!currentSeries.prNumber &&
			listedPr?.number &&
			listedPr.number !== currentSeries.prNumber
		) {
			branchController.updateBranchPrNumber(branch.id, currentSeries.name, listedPr.number);
		}
	});

	async function handleReloadPR() {
		await Promise.allSettled([prMonitor?.refresh(), checksMonitor?.update()]);
	}

	function handleOpenPR(pushBeforeCreate: boolean = false) {
		prDetailsModal?.show(pushBeforeCreate);
	}

	async function handleReopenPr() {
		if (!$pr) {
			return;
		}
		await $prService?.reopen($pr?.number);
		await Promise.allSettled([prMonitor?.refresh(), checksMonitor?.update()]);
	}

	function editTitle(title: string) {
		if (currentSeries?.name && title !== currentSeries.name) {
			branchController.updateSeriesName(branch.id, currentSeries.name, title);
		}
	}

	async function editDescription(description: string | undefined | null) {
		if (description) {
			await branchController.updateSeriesDescription(branch.id, currentSeries.name, description);
		}
	}

	async function toggleDescription() {
		descriptionVisible = !descriptionVisible;

		if (!descriptionVisible) {
			await branchController.updateSeriesDescription(branch.id, currentSeries.name, '');
		} else {
			await tick();
			seriesDescriptionEl?.focus();
		}
	}

	async function generateBranchName() {
		if (!aiGenEnabled || !currentSeries) return;

		let hunk_promises = currentSeries.patches.flatMap(async (p) => {
			let files = await listCommitFiles(project.id, p.id);
			return files.flatMap((f) =>
				f.hunks.map((h) => {
					return { filePath: f.path, diff: h.diff };
				})
			);
		});
		let hunks = (await Promise.all(hunk_promises)).flat();

		const prompt = promptService.selectedBranchPrompt(project.id);
		const messageResult = await aiService.summarizeBranch({
			hunks,
			branchTemplate: prompt
		});

		if (isFailure(messageResult)) {
			showError('Failed to generate branch name', messageResult.failure);

			return;
		}

		const message = messageResult.value;

		if (message && message !== currentSeries.name) {
			branchController.updateSeriesName(branch.id, currentSeries.name, message);
		}
	}
</script>

<AddSeriesModal bind:this={stackingAddSeriesModal} parentSeriesName={currentSeries.name} />

<SeriesHeaderContextMenu
	bind:this={stackingContextMenu}
	bind:contextMenuEl={kebabContextMenu}
	leftClickTrigger={kebabContextMenuTrigger}
	rightClickTrigger={seriesHeaderEl}
	headName={currentSeries.name}
	seriesCount={branch.series?.length ?? 0}
	{toggleDescription}
	description={currentSeries.description ?? ''}
	onGenerateBranchName={generateBranchName}
	onAddDependentSeries={() => stackingAddSeriesModal?.show()}
	onOpenInBrowser={() => {
		const url = forgeBranch?.url;
		if (url) openExternalUrl(url);
	}}
	hasForgeBranch={!!forgeBranch}
	prUrl={$pr?.htmlUrl}
	openPrDetailsModal={handleOpenPR}
	{branchType}
	onMenuToggle={(isOpen, isLeftClick) => {
		if (isLeftClick) {
			contextMenuOpened = isOpen;
		}
	}}
/>

<div
	role="article"
	class="branch-header"
	bind:this={seriesHeaderEl}
	oncontextmenu={(e) => {
		e.preventDefault();
		kebabContextMenu?.toggle(e);
	}}
>
	<Dropzones type="commit">
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
			{#if forgeBranch}
				<PopoverActionsItem
					icon="open-link"
					tooltip="Open in browser"
					onclick={() => {
						const url = forgeBranch?.url;
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
			<SeriesHeaderStatusIcon
				lineTop={isTopSeries ? false : true}
				icon={branchType === 'integrated' ? 'tick-small' : 'branch-small'}
				iconColor="var(--clr-core-ntrl-100)"
				color={lineColor}
				lineBottom={currentSeries.patches.length > 0 || branch.series.length > 1}
			/>
			<div class="branch-info__content">
				<div class="text-14 text-bold branch-info__name">
					{#if forgeBranch}
						<span class="remote-name">
							{$baseBranch.pushRemoteName ? `${$baseBranch.pushRemoteName} /` : 'origin /'}
						</span>
					{/if}
					<BranchLabel
						name={currentSeries.name}
						onChange={(name) => editTitle(name)}
						readonly={!!forgeBranch}
						onDblClick={() => {
							if (branchType !== 'integrated') {
								stackingContextMenu?.showSeriesRenameModal?.();
							}
						}}
					/>
				</div>
				{#if descriptionVisible}
					<div class="branch-info__description">
						<div class="branch-action__line" style:--bg-color={lineColor}></div>
						<SeriesDescription
							bind:textAreaEl={seriesDescriptionEl}
							value={currentSeries.description ?? ''}
							onBlur={(value) => editDescription(value)}
							onEmpty={() => toggleDescription()}
						/>
					</div>
				{/if}
			</div>
		</div>
		{#if ($prService && !hasNoCommits) || showCreateCloudBranch}
			<div class="branch-action">
				<div class="branch-action__line" style:--bg-color={lineColor}></div>
				<div class="branch-action__body">
					{#if $prService && !hasNoCommits}
						{#if $pr}
							<PullRequestCard
								upstreamName={currentSeries.name}
								reloadPR={handleReloadPR}
								reopenPr={handleReopenPr}
								openPrDetailsModal={handleOpenPR}
								pr={$pr}
								{checksMonitor}
							/>
						{:else}
							<Button
								style="ghost"
								wide
								outline
								disabled={currentSeries.patches.length === 0 || !$forge || !$prService}
								onclick={() => handleOpenPR(!forgeBranch)}
							>
								Create pull request
							</Button>
						{/if}
					{/if}

					{#if showCreateCloudBranch}
						<Button
							style="ghost"
							outline
							disabled={branch.commits.length === 0}
							onclick={() => {
								cloudBranchCreationService.createBranch(branch.id);
							}}>Publish Branch</Button
						>
					{/if}
				</div>
			</div>
		{/if}

		{#if $pr}
			<PrDetailsModal bind:this={prDetailsModal} type="display" pr={$pr} />
		{:else}
			<PrDetailsModal
				bind:this={prDetailsModal}
				type="preview-series"
				{currentSeries}
				stackId={branch.id}
			/>
		{/if}
	</Dropzones>
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
		padding-right: 14px;
		display: flex;
		justify-content: flex-start;
		align-items: center;

		.remote-name {
			min-width: max-content;
			padding: 0 0 0 2px;
			color: var(--clr-scale-ntrl-60);
		}
	}

	.branch-info__name {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		min-width: 0;
		flex-grow: 1;
	}

	.branch-info__content {
		overflow: hidden;
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 0;
		margin-left: -2px;
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
		margin: 0 22px 0 20px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>
