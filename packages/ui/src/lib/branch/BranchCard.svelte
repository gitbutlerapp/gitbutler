<script lang="ts">
	import ScrollableContainer from '@gitbutler/ui/scrolling/ScrollableContainer.svelte';
	import type { Snippet } from 'svelte';
	import type { Writable } from 'svelte/store';

	interface Props {
		isLaneCollapsed: Writable<boolean>;
		selectedForChanges?: boolean;

		branchHeader: Snippet;
		pullRequestCard?: Snippet;
		branchFiles: Snippet;
		commitList?: Snippet;
		branchFooter: Snippet;
	}

	const {
		isLaneCollapsed,
		selectedForChanges = false,

		branchHeader,
		pullRequestCard,
		branchFiles,
		commitList,
		branchFooter
	}: Props = $props();
</script>

{#if $isLaneCollapsed}
	<div class="collapsed-lane-container">
		{@render branchHeader()}
	</div>
{:else}
	<div class="resizer-wrapper">
		<div
			class="branch-card hide-native-scrollbar"
			data-tauri-drag-region
			class:target-branch={selectedForChanges}
		>
			<ScrollableContainer
				wide
				padding={{
					top: 12,
					bottom: 12
				}}
			>
				{@render branchHeader()}
				{#if pullRequestCard}
					{@render pullRequestCard()}
				{/if}

				<div class="card">
					<div class="branch-card__files">
						{@render branchFiles()}
					</div>

					<div class="card-commits">
						{#if commitList}
							{@render commitList()}
						{/if}
						{@render branchFooter()}
					</div>
				</div>
			</ScrollableContainer>
		</div>
	</div>
{/if}

<style lang="postcss">
	.resizer-wrapper {
		position: relative;
		display: flex;
		height: 100%;
	}
	.branch-card {
		height: 100%;
		position: relative;
		user-select: none;
		overflow-x: hidden;
		overflow-y: scroll;
	}

	.card {
		flex: 1;
	}

	.branch-card__files {
		display: flex;
		flex-direction: column;
		flex: 1;
		height: 100%;
	}

	/* COLLAPSED LANE */
	.collapsed-lane-container {
		display: flex;
		flex-direction: column;
		padding: 12px;
		height: 100%;
		border-right: 1px solid var(--clr-border-2);
	}
</style>
