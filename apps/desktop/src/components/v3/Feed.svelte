<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FeedItem from '$components/v3/FeedItem.svelte';
	import { Feed } from '$lib/feed/feed';
	import { onMount, tick } from 'svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const feed = new Feed(projectId);
	const combinedEntries = feed.combined;

	let viewport = $state<HTMLDivElement>();
	let topSentinel = $state<HTMLDivElement>();
	let canLoadMore = $state(false);
	let prevScrollHeight = $state<number>(0);

	async function loadMoreItems() {
		if (!canLoadMore || !viewport) return;
		canLoadMore = false;
		prevScrollHeight = viewport.scrollHeight;
		await feed.fetch();
		await tick();
		const newScrollHeight = viewport.scrollHeight;
		viewport.scrollTop = newScrollHeight - prevScrollHeight - 5;

		await tick();
		canLoadMore = true;
	}

	onMount(() => {
		if (viewport) {
			setTimeout(() => {
				viewport!.scrollTop = viewport!.scrollHeight;
				canLoadMore = true;
			}, 100);

			// Setup observer
			const observer = new IntersectionObserver(
				(entries) => {
					const first = entries[0];
					if (first?.isIntersecting) {
						loadMoreItems();
					}
				},
				{
					root: viewport,
					threshold: 0
				}
			);

			if (topSentinel) {
				observer.observe(topSentinel);
			}

			return () => {
				if (topSentinel) {
					observer.unobserve(topSentinel);
				}
			};
		}
	});
</script>

<div class="action-log-wrap">
	<div class="action-log">
		<div class="action-log__header">
			<h2 class="text-16 text-semibold">Butler Actions</h2>
		</div>
		<ConfigurableScrollableContainer bind:viewport>
			<div class="feed">
				{#each $combinedEntries as entry (entry.id)}
					<FeedItem {projectId} action={entry} />
				{/each}
				<div bind:this={topSentinel} style="height: 1px"></div>
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
