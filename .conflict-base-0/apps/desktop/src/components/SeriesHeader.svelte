<script lang="ts">
	import AddSeriesModal from '$components/AddSeriesModal.svelte';
	import BranchLabel from '$components/BranchLabel.svelte';
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import BranchReview from '$components/BranchReview.svelte';
	import BranchStatus from '$components/BranchStatus.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import DeleteBranchModal from '$components/DeleteBranchModal.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import SeriesDescription from '$components/SeriesDescription.svelte';
	import SeriesHeaderContextMenuContents from '$components/SeriesHeaderContextMenuContents.svelte';
	import SeriesHeaderStatusIcon from '$components/SeriesHeaderStatusIcon.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BranchStack } from '$lib/branches/branch';
	import { PatchSeries } from '$lib/branches/branch';
	import { parentBranch } from '$lib/branches/virtualBranchService';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { MoveCommitDzHandler } from '$lib/commits/dropHandler';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { FileService } from '$lib/files/fileService';
	import { closedStateSync } from '$lib/forge/closedStateSync.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext, getContextStore, inject } from '@gitbutler/shared/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { tick } from 'svelte';

	interface Props {
		projectId: string;
		branch: PatchSeries;
		isTopBranch: boolean;
	}

	const { projectId, branch, isTopBranch }: Props = $props();

	let descriptionVisible = $state(!!branch.description);

	const [aiService, promptService, fileService, forge] = inject(
		AIService,
		PromptService,
		FileService,
		DefaultForgeFactory
	);

	const stackStore = getContextStore(BranchStack);
	const stack = $derived($stackStore);

	const parent = $derived(
		parentBranch(
			branch,
			stack.validSeries.filter((b) => b.archived)
		)
	);

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const baseBranch = getContext(BaseBranch);

	const upstreamName = $derived(branch.upstreamReference ? branch.name : undefined);
	const forgeBranch = $derived(upstreamName ? forge.current.branch(upstreamName) : undefined);

	let stackingAddSeriesModal = $state<ReturnType<typeof AddSeriesModal>>();
	let kebabContextMenu = $state<ReturnType<typeof ContextMenu>>();
	let branchContextMenu = $state<ReturnType<typeof SeriesHeaderContextMenuContents>>();
	let kebabContextMenuTrigger = $state<HTMLButtonElement>();
	let seriesHeaderEl = $state<HTMLDivElement>();
	let seriesDescriptionEl = $state<HTMLTextAreaElement>();
	let contextMenuOpened = $state(false);

	const topPatch = $derived(branch?.patches[0]);
	const branchType = $derived<CommitStatusType>(topPatch?.status ?? 'LocalOnly');
	const lineColor = $derived(getColorFromBranchType(branchType));
	const hasNoCommits = $derived(branch.upstreamPatches.length === 0 && branch.patches.length === 0);
	const parentIsPushed = $derived(!!parent?.upstreamReference);
	const parentIsIntegrated = $derived(!!parent?.integrated);
	const hasParent = $derived(!!parent);
	const isPushed = $derived(!!branch.upstreamReference);

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const forgeListing = $derived(forge.current.listService);
	const listedPrResult = $derived(
		upstreamName ? forgeListing?.getByBranch(projectId, upstreamName) : undefined
	);
	const listedPr = $derived(listedPrResult?.current.data);
	const prNumber = $derived(branch.prNumber);

	const prService = $derived(forge.current.prService);
	const prResult = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prResult?.current.data);
	const mergedIncorrectly = $derived(
		(pr?.merged && pr.baseBranch !== baseBranch.shortName) || false
	);

	const stackService = getContext(StackService);
	const [updateBranchPrNumber, prNumberUpdate] = stackService.updateBranchPrNumber;
	const [updateBranchNameMutation] = stackService.updateBranchName;
	const [updateBranchDescription] = stackService.updateBranchDescription;

	/**
	 * We are starting to store pull request id's locally so if we find one that does not have
	 * one locally stored then we set it once.
	 *
	 * TODO: Remove this after transition is complete.
	 */
	let count = 0;
	$effect(() => {
		if (
			forge.current.name === 'github' &&
			!branch.prNumber &&
			listedPr?.number &&
			listedPr.number !== branch.prNumber &&
			prNumberUpdate.current.isUninitialized
		) {
			if (count++) return;
			updateBranchPrNumber({
				projectId: projectId,
				stackId: stack.id,
				branchName: branch.name,
				prNumber: listedPr.number
			});
		}
	});

	function updateBranchName(title: string) {
		if (branch?.name && title !== branch.name) {
			updateBranchNameMutation({
				projectId: projectId,
				stackId: stack.id,
				branchName: branch.name,
				newName: title
			});
		}
	}

	async function editDescription(description: string | undefined | null) {
		if (description) {
			await updateBranchDescription({
				projectId: projectId,
				stackId: stack.id,
				branchName: branch.name,
				description: description
			});
		}
	}

	async function toggleDescription() {
		descriptionVisible = !descriptionVisible;

		if (!descriptionVisible) {
			await updateBranchDescription({
				projectId: projectId,
				stackId: stack.id,
				branchName: branch.name,
				description: ''
			});
		} else {
			await tick();
			seriesDescriptionEl?.focus();
		}
	}

	async function generateBranchName() {
		if (!aiGenEnabled || !branch) return;

		let hunk_promises = branch.patches.flatMap(async (p) => {
			let files = await fileService.listCommitFiles(projectId, p.id);
			return files.flatMap((f) =>
				f.hunks.map((h) => {
					return { filePath: f.path, diff: h.diff };
				})
			);
		});
		let hunks = (await Promise.all(hunk_promises)).flat();

		const prompt = promptService.selectedBranchPrompt(projectId);
		const newBranchName = await aiService.summarizeBranch({
			hunks,
			branchTemplate: prompt
		});

		if (newBranchName && newBranchName !== branch.name) {
			updateBranchNameMutation({
				projectId: projectId,
				stackId: stack.id,
				branchName: branch.name,
				newName: newBranchName
			});
		}
	}

	closedStateSync(reactive(() => branch));

	const dzHandler = $derived(new MoveCommitDzHandler(stackService, stack, projectId));

	let renameBranchModal = $state<BranchRenameModal>();
	let deleteBranchModal = $state<DeleteBranchModal>();
</script>

<AddSeriesModal bind:this={stackingAddSeriesModal} parentSeriesName={branch.name} />

<ContextMenu
	testId={TestId.BranchHeaderContextMenu}
	bind:this={kebabContextMenu}
	leftClickTrigger={kebabContextMenuTrigger}
	rightClickTrigger={seriesHeaderEl}
	ontoggle={(isOpen, isLeftClick) => {
		if (isLeftClick) {
			contextMenuOpened = isOpen;
		}
	}}
>
	<SeriesHeaderContextMenuContents
		{projectId}
		stackId={stack.id}
		bind:this={branchContextMenu}
		branchName={branch.name}
		seriesCount={stack.validSeries?.length ?? 0}
		{isTopBranch}
		{toggleDescription}
		descriptionString={branch.description ?? ''}
		onGenerateBranchName={generateBranchName}
		onAddDependentSeries={() => stackingAddSeriesModal?.show()}
		onOpenInBrowser={() => {
			const url = forgeBranch?.url;
			if (url) openExternalUrl(url);
		}}
		{isPushed}
		{pr}
		{branchType}
		showBranchRenameModal={() => renameBranchModal?.show()}
		showDeleteBranchModal={() => deleteBranchModal?.show()}
	/>
</ContextMenu>

<BranchRenameModal
	{projectId}
	stackId={stack.id}
	branchName={branch.name}
	bind:this={renameBranchModal}
	{isPushed}
/>
<DeleteBranchModal
	{projectId}
	stackId={stack.id}
	branchName={branch.name}
	bind:this={deleteBranchModal}
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
	<Dropzone handlers={[dzHandler]}>
		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Move here" />
		{/snippet}
		<PopoverActionsContainer class="branch-actions-menu" stayOpen={contextMenuOpened}>
			{#if isTopBranch}
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
				lineTop={isTopBranch ? false : true}
				icon={branchType === 'Integrated' ? 'tick-small' : 'branch-small'}
				iconColor="var(--clr-core-ntrl-100)"
				color={lineColor}
			/>
			<div class="branch-info__content">
				<div class="text-14 text-bold branch-info__name">
					{#if isPushed}
						<span class="remote-name">
							{baseBranch.pushRemoteName ? `${baseBranch.pushRemoteName} /` : 'origin /'}
						</span>
					{/if}
					<BranchLabel
						name={branch.name}
						onChange={(name) => updateBranchName(name)}
						readonly={isPushed}
						onDblClick={() => {
							if (isPushed) {
								renameBranchModal?.show();
							}
						}}
					/>
				</div>
				{#if descriptionVisible}
					<div class="branch-info__description">
						<div class="branch-action__line" style:--bg-color={lineColor}></div>
						<SeriesDescription
							bind:textAreaEl={seriesDescriptionEl}
							value={branch.description ?? ''}
							onBlur={(value) => editDescription(value)}
							onEmpty={() => toggleDescription()}
						/>
					</div>
				{/if}
			</div>
		</div>
		{#if !hasNoCommits}
			<div class="branch-review-section">
				<div class="branch-action__line" style:--bg-color={lineColor}></div>
				<div class="branch-review-container">
					<BranchReview {projectId} stackId={stack.id} branchName={branch.name}>
						{#snippet branchStatus()}
							<BranchStatus
								{mergedIncorrectly}
								{isPushed}
								{hasParent}
								{parentIsPushed}
								{parentIsIntegrated}
							/>
						{/snippet}
					</BranchReview>
				</div>
			</div>
		{/if}
	</Dropzone>
</div>

<style lang="postcss">
	.branch-review-section {
		display: flex;
	}

	.branch-review-container {
		flex-grow: 1;
		padding: 0 14px 14px 0;
	}

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

	.branch-action__line {
		min-width: 2px;
		margin: 0 22px 0 20px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>
