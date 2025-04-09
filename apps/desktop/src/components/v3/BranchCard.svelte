<script lang="ts">
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import BranchDividerLine from '$components/v3/BranchDividerLine.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { PushStatus } from '$lib/stacks/stack';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		trackingBranch?: string;
		reviewId?: string;
		prNumber?: number;
		pushStatus: PushStatus;
		isConflicted: boolean;
		lastUpdatedAt: number;
		first: boolean;
		last: boolean;
		isNewBranch: boolean;
		lineColor: string;
		iconName: keyof typeof iconsJson;
		commitList?: Snippet;
		menu?: Snippet<
			[
				{
					onToggle: (open: boolean, isLeftClick: boolean) => void;
				}
			]
		>;
	}

	let {
		projectId,
		stackId,
		branchName,
		trackingBranch,
		reviewId,
		prNumber,
		pushStatus,
		isConflicted,
		lastUpdatedAt,
		lineColor,
		iconName,
		first,
		isNewBranch,
		commitList,
		menu
	}: Props = $props();

	const [uiState] = inject(UiState);

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
	const selected = $derived(selection.current?.branchName === branchName);

	let contextMenu: ContextMenu;
</script>

{#if !first}
	<BranchDividerLine {lineColor} />
{/if}
<div class="branch-card" class:selected data-series-name={branchName}>
	<BranchHeader
		{branchName}
		{projectId}
		{stackId}
		{lineColor}
		{iconName}
		bind:el={rightClickTrigger}
		bind:menuBtnEl={leftClickTrigger}
		{trackingBranch}
		{isMenuOpenByBtn}
		{isMenuOpenByMouse}
		selected={selected && selection.current?.commitId === undefined}
		isTopBranch={first}
		{isNewBranch}
		readonly={!!trackingBranch}
		onclick={() => {
			const stackState = uiState.stack(stackId);
			stackState.selection.set({ branchName });
			stackState.activeSelectionId.set({ type: 'branch', branchName, stackId });
			uiState.project(projectId).drawerPage.set('branch');
		}}
		onMenuBtnClick={() => contextMenu.toggle()}
		onContextMenu={(e) => contextMenu.toggle(e)}
	>
		{#snippet details()}
			<div class="text-11 branch-header__details">
				<span class="branch-header__item">
					<BranchBadge {pushStatus} unstyled />
				</span>
				<span class="branch-header__divider">•</span>

				{#if isConflicted}
					<span class="branch-header__item branch-header__item--conflict"> Has conflicts </span>
					<span class="branch-header__divider">•</span>
				{/if}

				<span class="branch-header__item">
					{getTimeAgo(new Date(lastUpdatedAt))}
				</span>

				{#if reviewId || prNumber}
					<span class="branch-header__divider">•</span>
					{#if reviewId}
						<ReviewBadge brId={reviewId} brStatus="unknown" />
					{/if}
					{#if prNumber}
						<ReviewBadge {prNumber} prStatus="unknown" />
					{/if}
				{/if}
			</div>
		{/snippet}
	</BranchHeader>

	{#if !isNewBranch}
		{@render commitList?.()}
	{/if}
</div>

<ContextMenu
	testId={TestId.BranchHeaderContextMenu}
	bind:this={contextMenu}
	{leftClickTrigger}
	{rightClickTrigger}
	ontoggle={(isOpen, isLeftClick) => {
		onToggle?.(isOpen, isLeftClick);
	}}
>
	{@render menu?.({
		onToggle
	})}
</ContextMenu>

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
