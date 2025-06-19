<script lang="ts">
	import FeedItem from '$components/v3/FeedItem.svelte';
	import { Feed } from '$lib/feed/feed';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const feed = new Feed(projectId);
	const combinedEntries = feed.combined;
</script>

<div class="action-log-wrap">
	<div class="action-log">
		<div class="action-log__header">
			<h2 class="text-16 text-semibold">Butler Actions</h2>
		</div>
		<div class="scrollable">
			{#each $combinedEntries as entry, idx (entry.id)}
				<FeedItem
					{projectId}
					action={entry}
					last={$combinedEntries.length - 1 === idx}
					loadNextPage={() => feed.fetch()}
				/>
			{/each}
		</div>
	</div>
</div>

<style lang="postcss">
	.action-log-wrap {
		display: flex;

		min-width: 0;
		overflow: hidden;

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
