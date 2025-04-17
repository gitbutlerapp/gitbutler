<script lang="ts">
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import BranchDividerLine from '$components/v3/BranchDividerLine.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	type Props = {
		type: 'draft-branch' | 'normal-branch' | 'stack-branch';
		projectId: string;
		branchName: string;
		isCommitting?: boolean;
		header?: Snippet;
	} & (
		| {
				type: 'draft-branch';
		  }
		| {
				type: 'normal-branch';
				commitList?: Snippet;
		  }
		| {
				type: 'stack-branch';
				stackId: string;
				lineColor: string;
				first: boolean;
				commitList?: Snippet;
				menu?: Snippet<
					[
						{
							onToggle: (open: boolean, isLeftClick: boolean) => void;
						}
					]
				>;
				headCommit: string;
		  }
	);

	let { header, branchName, ...args }: Props = $props();

	const [uiState] = inject(UiState);

	const selection = $derived(
		args.type === 'stack-branch' ? uiState.stack(args.stackId).selection.get() : undefined
	);

	const selected = $derived(selection?.current?.branchName === branchName);

	let contextMenu: ContextMenu | undefined = $state();
</script>

{#if args.type === 'stack-branch' && !args.first}
	<BranchDividerLine lineColor={args.lineColor} />
{/if}
<div
	class="branch-card"
	class:selected
	class:draft={args.type === 'draft-branch'}
	data-series-name={branchName}
>
	{#if args.type === 'stack-branch'}
		<BranchHeader
			type="stack-branch"
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
				if (isCommitting) {
					uiState.stack(args.stackId).selection.set({
						branchName,
						commitId: args.headCommit
					});
				} else {
					uiState.stack(args.stackId).selection.set({ branchName });
					uiState.project(projectId).drawerPage.set('branch');
				}
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
	{:else if args.type === 'normal-branch'}
		<BranchHeader
			type="normal-branch"
			{branchName}
			{projectId}
			lineColor={getColorFromBranchType('LocalOnly')}
			{iconName}
			bind:el={rightClickTrigger}
			bind:menuBtnEl={leftClickTrigger}
			trackingBranch={args.trackingBranch}
			{isCommitting}
			selected={selected && selection?.current?.commitId === undefined}
			readonly
			onclick={() => {
				uiState.project(projectId).branchesSelection.set({
					branchName
				});
			}}
		>
			{#snippet details()}
				<div class="text-11 branch-header__details">
					{#if args.lastUpdatedAt}
						<span class="branch-header__item">
							{getTimeAgo(new Date(args.lastUpdatedAt))}
						</span>
					{/if}
				</div>
			{/snippet}
		</BranchHeader>
	{:else}
		<BranchHeader
			type="draft-branch"
			{branchName}
			{projectId}
			{iconName}
			readonly={false}
			lineColor="var(--clr-commit-local)"
		/>
	{/if}

	{#if args.type !== 'draft-branch'}
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
</style>
