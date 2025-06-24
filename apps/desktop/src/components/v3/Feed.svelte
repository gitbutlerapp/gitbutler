<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FeedItem from '$components/v3/FeedItem.svelte';
	import { ActionService } from '$lib/actions/actionService.svelte';
	import { Feed } from '$lib/feed/feed';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { onMount } from 'svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const feed = new Feed(projectId);
	const combinedEntries = feed.combined;
	const actionService = getContext(ActionService);
	const [figureItOut, figuringThings] = actionService.figureOutTheCommits;

	let viewport = $state<HTMLDivElement>();
	onMount(() => {
		if (viewport) {
			setTimeout(() => {
				viewport!.scrollTop = viewport!.scrollHeight;
			}, 100);
		}
	});
</script>

<div class="action-log-wrap">
	<div class="action-log">
		<div class="action-log__header">
			<h2 class="text-16 text-semibold">Butler Actions</h2>
			<Button
				kind="outline"
				icon="ai-small"
				size="tag"
				loading={figuringThings.current.isLoading}
				onclick={() => {
					figureItOut({ projectId });
				}}>idk</Button
			>
		</div>
		<ConfigurableScrollableContainer bind:viewport>
			<div class="feed">
				{#each $combinedEntries as entry (entry.id)}
					<FeedItem {projectId} action={entry} />
				{/each}
			</div>
		</ConfigurableScrollableContainer>
	</div>
</div>

<style lang="postcss">
	.action-log-wrap {
		display: flex;
		position: relative;

		min-width: 0;
		height: 100%;
		overflow: hidden;

		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.action-log {
		display: flex;
		flex-direction: column;
		width: 100%;

		height: 100%;
	}

	.action-log__header {
		display: flex;
		position: sticky;
		top: 0;
		width: 100%;
		padding: 16px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.feed {
		display: flex;
		flex-direction: column-reverse;
		padding: 16px;

		gap: 20px;
	}
</style>
