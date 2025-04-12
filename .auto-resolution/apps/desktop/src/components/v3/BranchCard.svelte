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

	type Props = {
		projectId: string;
		branchName: string;
		iconName: keyof typeof iconsJson;
		draft?: boolean;
		isCommitting?: boolean;
	} & (
		| { draft: true; description?: Snippet }
		| {
				draft?: false | undefined;
				stackId: string;
				first: boolean;
				last: boolean;
				trackingBranch?: string;
				reviewId?: string;
				prNumber?: number;
				lineColor: string;
				isNewBranch: boolean;
				pushStatus: PushStatus;
				isConflicted: boolean;
				lastUpdatedAt: number;
				commitList?: Snippet;
				menu?: Snippet<
					[
						{
							onToggle: (open: boolean, isLeftClick: boolean) => void;
						}
					]
				>;
		  }
	);

	let { projectId, branchName, iconName, isCommitting, ...args }: Props = $props();

	const [uiState] = inject(UiState);

	const selection = $derived(!args.draft ? uiState.stack(args.stackId).selection.get() : undefined);

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
	const selected = $derived(selection?.current?.branchName === branchName);

	let contextMenu: ContextMenu | undefined = $state();
</script>

{#if !args.draft && !args.first}
	<BranchDividerLine lineColor={args.lineColor} />
{/if}
<div class="branch-card" class:selected class:draft={args.draft} data-series-name={branchName}>
	{#if !args.draft}
		<BranchHeader
			draft={args.draft}
			{branchName}
			{projectId}
			stackId={args.stackId}
			lineColor={args.lineColor}
			{iconName}
			bind:el={rightClickTrigger}
			bind:menuBtnEl={leftClickTrigger}
			trackingBranch={args.trackingBranch}
			{isMenuOpenByBtn}
			{isMenuOpenByMouse}
			{isCommitting}
			selected={selected && selection?.current?.commitId === undefined}
			isTopBranch={args.first}
			isNewBranch={args.isNewBranch}
			readonly={!!args.trackingBranch}
			onclick={() => {
				const stackState = uiState.stack(args.stackId);
				stackState.selection.set({ branchName });
				stackState.activeSelectionId.set({ type: 'branch', branchName, stackId: args.stackId });
				uiState.project(projectId).drawerPage.set('branch');
			}}
			onMenuBtnClick={() => contextMenu?.toggle()}
			onContextMenu={(e) => contextMenu?.toggle(e)}
		>
			{#snippet details()}
				<div class="text-11 branch-header__details">
					<span class="branch-header__item">
						<BranchBadge pushStatus={args.pushStatus} unstyled />
					</span>
					<span class="branch-header__divider">•</span>

					{#if args.isConflicted}
						<span class="branch-header__item branch-header__item--conflict"> Has conflicts </span>
						<span class="branch-header__divider">•</span>
					{/if}

					{#if args.lastUpdatedAt}
						<span class="branch-header__item">
							{getTimeAgo(new Date(args.lastUpdatedAt))}
						</span>
					{/if}

					{#if args.reviewId || args.prNumber}
						<span class="branch-header__divider">•</span>
						<div class="branch-header__review-badges">
							{#if args.reviewId}
								<ReviewBadge brId={args.reviewId} brStatus="unknown" />
							{/if}
							{#if args.prNumber}
								<ReviewBadge prNumber={args.prNumber} prStatus="unknown" />
							{/if}
						</div>
					{/if}
				</div>
			{/snippet}
		</BranchHeader>
		<ContextMenu
			testId={TestId.BranchHeaderContextMenu}
			bind:this={contextMenu}
			{leftClickTrigger}
			{rightClickTrigger}
			ontoggle={(isOpen, isLeftClick) => {
				onToggle?.(isOpen, isLeftClick);
			}}
		>
			{@render args.menu?.({
				onToggle
			})}
		</ContextMenu>
	{:else}
		<BranchHeader
			draft={true}
			{branchName}
			{projectId}
			{iconName}
			readonly={false}
			lineColor="var(--clr-commit-local)"
		/>
	{/if}

	{#if !args.draft}
		{@render args.commitList?.()}
	{/if}
</div>

<style>
	.branch-card {
		display: flex;
		flex-direction: column;
		width: 100%;
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
		&.draft {
			border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		}
	}

	.branch-header__details {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
		margin-left: 4px;
	}

	.branch-header__review-badges {
		display: flex;
		gap: 3px;
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
