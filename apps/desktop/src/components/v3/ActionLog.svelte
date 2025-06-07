<script lang="ts">
	import { ButlerAction } from '$lib/actions/types';
	import { Feed } from '$lib/feed/feed';
	import { Snapshot } from '$lib/history/types';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		selectionId: SelectionId;
	};

	const { projectId }: Props = $props();

	const feed = new Feed(projectId);
	const combinedEntries = feed.combined;

	// function loadNextPage() {
	// 	feed.fetch();
	// }
</script>

<div class="action-log-wrap">
	<div class="action-log">
		<div class="action-log__header">
			<h2 class="text-16 text-semibold">Butler Actions</h2>
		</div>
		<div class="scrollable">
			{#each $combinedEntries as entry}
				<!-- {entry.id} -->
				{#if entry instanceof Snapshot}
					<div>{entry.createdAt} snapshot</div>
				{:else if entry instanceof ButlerAction}
					<div>{entry.createdAt} action</div>
				{/if}
			{/each}
		</div>
	</div>
</div>

<style lang="postcss">
	.action-log-wrap {
		flex-grow: 1;

		overflow: hidden;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.action-log__header {
		padding: 16px;

		border-bottom: 1px solid var(--clr-border-2);
	}

	.action-log {
		display: flex;
		flex-direction: column;

		height: 100%;
	}

	.scrollable {
		display: flex;
		flex-grow: 1;
		flex-direction: column-reverse;

		padding: 16px;

		overflow: auto;

		gap: 20px;
	}
</style>
