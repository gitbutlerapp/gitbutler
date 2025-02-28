<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import ScrollableContainer from '$components/ScrollableContainer.svelte';
	import Branch from '$components/v3/Branch.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import type { Snippet } from 'svelte';

	interface Props {
		stackId: string;
		projectId: string;
		selectedBranchName: string;
		selectedCommitId?: string;
		children: Snippet;
	}

	const { stackId, projectId, selectedBranchName, selectedCommitId, children }: Props = $props();

	const [stackService] = inject(StackService);
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	const stackBranchWidthKey = $derived('defaultStackBranchWidth_ ' + projectId);
	const result = $derived(stackService.branches(projectId, stackId).current);

	let resizeStackBranches = $state<HTMLElement>();
	let stackBranchWidth = $derived(persisted<number>(22.5, stackBranchWidthKey));
</script>

<div class="branch-view">
	<ReduxResult {result}>
		{#snippet children(branches)}
			<div class="branches" bind:this={resizeStackBranches} style:width={$stackBranchWidth + 'rem'}>
				<Resizer
					viewport={resizeStackBranches}
					direction="right"
					minWidth={22.5}
					zIndex="var(--z-modal)"
					onWidth={(value) => {
						$stackBranchWidth = value / (16 * $userSettings.zoom);
					}}
				/>
				{#if stackId && branches.length >= 0}
					<ScrollableContainer wide>
						<div class="branch-scroll-container">
							{#each branches as branch, i (branch.name)}
								{@const first = i === 0}
								{@const last = i === branches.length - 1}
								<Branch
									{projectId}
									{stackId}
									branchName={branch.name}
									selected={selectedBranchName === branch.name}
									{selectedCommitId}
									{first}
									{last}
								/>
							{/each}
						</div>
					</ScrollableContainer>
				{/if}
			</div>
		{/snippet}
	</ReduxResult>
	{@render children()}
</div>

<style>
	.branch-view {
		position: relative;
		height: 100%;
		flex-grow: 1;
		display: flex;
		border-radius: 0 var(--radius-ml) var(--radius-ml);
	}

	.branches {
		position: relative;
		display: flex;
		width: 22.5rem;
		flex-direction: column;
		overflow: hidden;

		background-color: transparent;
		opacity: 1;
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
		border-right: 1px solid var(--clr-border-2);
	}

	.branch-scroll-container {
		padding: 14px;
	}
</style>
