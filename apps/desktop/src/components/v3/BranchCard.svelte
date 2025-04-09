<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import BranchDividerLine from '$components/v3/BranchDividerLine.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { CommitStatus } from '$lib/commits/commit';
	import type { ContextTrigger } from '@gitbutler/ui/ContextMenu.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		first: boolean;
		last: boolean;
		isNewBranch: boolean;
		commitList: Snippet;
		contextMenu: Snippet<
			[
				{
					branchName: string;
					trackingBranch?: string;
					leftClickTrigger?: HTMLElement;
					rightClickTrigger?: HTMLElement;
					branchType: CommitStatus;
					onToggle: (open: boolean, isLeftClick: boolean) => void;
					addListener?: (callback: ContextTrigger) => void;
				}
			]
		>;
	}

	let { projectId, stackId, branchName, first, isNewBranch, commitList, contextMenu }: Props =
		$props();

	const [stackService, uiState] = inject(StackService, UiState);

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branchDetailsResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const commitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0));

	const selection = $derived(uiState.stack(stackId).selection.get());

	let rightClickTrigger = $state<HTMLDivElement>();
	let leftClickTrigger = $state<HTMLButtonElement>();

	let isMenuOpenByBtn = $state(false);
	let isMenuOpenByMouse = $state(false);

	function onToggle(isOpen: boolean, isLeftClick: boolean) {
		if (isLeftClick) {
			isMenuOpenByBtn = isOpen;
		} else {
			isMenuOpenByMouse = isLeftClick;
		}
	}
</script>

<ReduxResult
	{stackId}
	{projectId}
	result={combineResults(branchResult.current, branchDetailsResult.current, commitResult.current)}
>
	{#snippet children([branch, branchDetails, commit], { projectId, stackId })}
		{@const branchName = branch.name}
		{@const selected = selection.current?.branchName === branch.name}
		{@const callbacks: ((e: MouseEvent | undefined, item: any)=>void)[] = []}
		{@const trigger = {
			// Super hacky thing, but gets the job done. This code makes
			// it possible for the context menu component to be injected
			// into this component as a snippet. A click on the header
			// must weave its way through to back to the header, but as
			// a result of the status of the context menu.
			addListener: (callback: ContextTrigger) => {
				callbacks.push(callback);
				return () => {
					callbacks.splice(callbacks.indexOf(callback), 1);
				};
			},
			fire: (e?: MouseEvent, item?: any) => {
				for (const callback of callbacks) {
					callback(e, item);
				}
			}
		}}
		{#if !first}
			<BranchDividerLine {commit} />
		{/if}
		<div class="branch-card" class:selected data-series-name={branchName}>
			<BranchHeader
				{projectId}
				{stackId}
				{branch}
				bind:el={rightClickTrigger}
				bind:menuBtnEl={leftClickTrigger}
				{isMenuOpenByBtn}
				{isMenuOpenByMouse}
				selected={selected && selection.current?.commitId === undefined}
				isTopBranch={first}
				{isNewBranch}
				readonly={!!branch.remoteTrackingBranch}
				onclick={() => {
					const stackState = uiState.stack(stackId);
					stackState.selection.set({ branchName });
					stackState.activeSelectionId.set({ type: 'branch', branchName, stackId });
					uiState.project(projectId).drawerPage.set('branch');
				}}
				onMenuBtnClick={() => trigger.fire()}
				onContextMenu={(e) => trigger.fire(e)}
			>
				{#snippet details()}
					<div class="text-11 branch-header__details">
						<span class="branch-header__item">
							<BranchBadge pushStatus={branchDetails.pushStatus} unstyled />
						</span>
						<span class="branch-header__divider">•</span>

						{#if branchDetails.isConflicted}
							<span class="branch-header__item branch-header__item--conflict"> Has conflicts </span>
							<span class="branch-header__divider">•</span>
						{/if}

						<span class="branch-header__item">
							{getTimeAgo(new Date(branchDetails.lastUpdatedAt))}
						</span>

						{#if branch.reviewId || branch.prNumber}
							<span class="branch-header__divider">•</span>
							{#if branch.reviewId}
								<ReviewBadge brId={branch.reviewId} brStatus="unknown" />
							{/if}
							{#if branch.prNumber}
								<ReviewBadge prNumber={branch.prNumber} prStatus="unknown" />
							{/if}
						{/if}
					</div>
				{/snippet}
			</BranchHeader>

			{#if !isNewBranch}
				{@render commitList()}
			{/if}
		</div>

		{@render contextMenu?.({
			branchType: commit?.state.type || 'LocalOnly',
			branchName: branch.name,
			trackingBranch: branch.remoteTrackingBranch || undefined,
			leftClickTrigger,
			rightClickTrigger,
			onToggle,
			// Note that the context menu must on render call this listener
			// to be notified of toggle clicks.
			addListener: trigger.addListener
		})}
	{/snippet}
</ReduxResult>

<style>
	.branch-card {
		display: flex;
		flex-direction: column;
		width: 100%;
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
	}

	.branch-header__details {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
		margin-left: 4px;
	}

	.branch-header__item {
		white-space: nowrap;
		color: var(--clr-text-2);
	}

	.branch-header__item--conflict {
		color: var(--clr-theme-err-element);
	}

	.branch-header__divider {
		color: var(--clr-text-3);
	}
</style>
