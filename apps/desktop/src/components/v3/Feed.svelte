<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FeedItem from '$components/v3/FeedItem.svelte';
	import { Feed } from '$lib/feed/feed';
	import Button from '@gitbutler/ui/Button.svelte';
	import { onMount, tick } from 'svelte';

	type Props = {
		projectId: string;
		onCloseClick: () => void;
	};

	const { projectId, onCloseClick }: Props = $props();

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
			<h2 class="flex-1 text-14 text-semibold">Butler Actions</h2>
			<Button icon="cross" kind="ghost" onclick={onCloseClick} />
		</div>
		<ConfigurableScrollableContainer childrenWrapHeight="100%" bind:viewport>
			<!-- {console.log('Combined Entries:', $combinedEntries)} -->
			{#if $combinedEntries.length === 0}
				<div class="text-14 text-center text-muted">No actions yet</div>
			{:else}
				<div class="feed">
					{#each $combinedEntries as entry (entry.id)}
						<FeedItem {projectId} action={entry} />
					{/each}
					<div bind:this={topSentinel} style="height: 1px"></div>
				</div>
			{/if}
		</ConfigurableScrollableContainer>
	</div>
</div>

<style lang="postcss">
	.action-log-wrap {
		display: flex;
		position: relative;
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
		align-items: center;
		width: 100%;
		padding: 8px 8px 8px 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.feed {
		display: flex;
		flex-direction: column-reverse;
		min-height: 100%;
	}
</style>
