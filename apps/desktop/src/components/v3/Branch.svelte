<script lang="ts">
	import BranchDividerLine from './BranchDividerLine.svelte';
	import CommitRow from './CommitRow.svelte';
	import EmptyBranch from './EmptyBranch.svelte';
	import NewBranchModal from './NewBranchModal.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesHeaderContextMenu from '$components/SeriesHeaderContextMenu.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import { getForge } from '$lib/forge/interface/forge';
	import { commitPath, createCommitPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import type { StackBranch } from '$lib/branches/v3';
	import { goto } from '$app/navigation';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		first: boolean;
		last: boolean;
		selected: boolean;
		selectedCommitId?: string;
	}

	let {
		projectId,
		stackId,
		branchName,
		first,
		last: lastBranch,
		selected,
		selectedCommitId = $bindable()
	}: Props = $props();

	const [stackService] = inject(StackService);

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const commitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0));

	const forge = getForge();
	const forgeBranch = $derived($forge?.branch(branchName));

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();
	let stackingContextMenu = $state<ReturnType<typeof SeriesHeaderContextMenu>>();
	let kebabContextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabContextMenuTrigger = $state<HTMLButtonElement>();
	let branchElement = $state<HTMLDivElement>();
	let contextMenuOpened = $state(false);

	export function allPreviousSeriesHavePrNumber(
		_seriesName: string,
		_validSeries: StackBranch[]
	): boolean {
		// Stub as a reminder this must be implemented.
		throw new Error('Not implemented!');
	}
	async function generateBranchName(_branch: StackBranch) {
		// Stub as a reminder this must be implemented.
		throw new Error('Not implemented!');
	}
</script>

<ReduxResult
	result={combineResults(branchResult.current, branchesResult.current, commitResult.current)}
>
	{#snippet children([branch, branches, commit])}
		{@const parentIsPushed = !!parent}
		{@const hasParent = !!parent}
		{#if !first}
			<BranchDividerLine topPatchStatus={commit?.state.type ?? 'LocalOnly'} />
		{/if}
		<div class="branch" class:selected data-series-name={branchName} bind:this={branchElement}>
			<BranchHeader
				{projectId}
				{stackId}
				{branch}
				isTopBranch={first}
				readonly={!!forgeBranch}
				onLabelDblClick={() => stackingContextMenu?.showSeriesRenameModal?.()}
			>
				{#snippet children()}
					<PopoverActionsContainer class="branch-actions-menu" stayOpen={contextMenuOpened}>
						{#if first}
							<PopoverActionsItem
								icon="plus-small"
								tooltip="Add dependent branch"
								onclick={() => {
									newBranchModal?.show();
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
					<NewBranchModal
						{projectId}
						{stackId}
						bind:this={newBranchModal}
						parentSeriesName={branch.name}
					/>

					<SeriesHeaderContextMenu
						bind:this={stackingContextMenu}
						bind:contextMenuEl={kebabContextMenu}
						{stackId}
						leftClickTrigger={kebabContextMenuTrigger}
						rightClickTrigger={branchElement}
						headName={branch.name}
						seriesCount={branches.length}
						isTopBranch={true}
						toggleDescription={async () => {}}
						description={branch.description ?? ''}
						onGenerateBranchName={() => generateBranchName(branch)}
						onAddDependentSeries={() => newBranchModal?.show()}
						onOpenInBrowser={() => {
							const url = forgeBranch?.url;
							if (url) openExternalUrl(url);
						}}
						openPrDetailsModal={() => {}}
						hasForgeBranch={!!forgeBranch}
						onCreateNewPr={() => goto(createCommitPath(projectId, stackId, branchName))}
						branchType={commit?.state.type || 'LocalOnly'}
						onMenuToggle={(isOpen, isLeftClick) => {
							if (isLeftClick) {
								contextMenuOpened = isOpen;
							}
						}}
						{parentIsPushed}
						{hasParent}
					/>
				{/snippet}
			</BranchHeader>
			<BranchCommitList {projectId} {stackId} {branchName} {selectedCommitId}>
				{#snippet emptyPlaceholder()}
					<EmptyBranch {lastBranch} />
				{/snippet}
				{#snippet upstreamTemplate({ commit, commitKey, first, lastCommit, selected })}
					<CommitRow
						{projectId}
						{commitKey}
						{first}
						{lastCommit}
						{commit}
						{selected}
						onclick={() => goto(commitPath(projectId, commitKey))}
					/>
				{/snippet}
				{#snippet localAndRemoteTemplate({
					commit,
					commitKey,
					first,
					lastCommit: lastCommit,
					selected
				})}
					<CommitRow
						{projectId}
						{commitKey}
						{first}
						{lastCommit}
						{lastBranch}
						{commit}
						{selected}
						onclick={() => goto(commitPath(projectId, commitKey))}
					/>
				{/snippet}
			</BranchCommitList>
		</div>
	{/snippet}
</ReduxResult>

<style>
	.branch {
		display: flex;
		flex-direction: column;
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
	}
</style>
