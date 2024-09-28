<script lang="ts">
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		title: string | undefined;
		children: Snippet;
	}

	const { title, children }: Props = $props();
</script>

<section class="content-wrapper">
	<ScrollableContainer>
		<div class="drag-region" data-tauri-drag-region>
			<div class="content" data-tauri-drag-region>
				{#if title}
					<h1 class="title text-head-24">
						{title}
					</h1>
				{/if}
				{@render children()}
			</div>
		</div>
	</ScrollableContainer>
</section>

<style lang="postcss">
	.content-wrapper {
		user-select: none;
		width: 100%;
		height: 100%;
		flex: 1;
		background-color: var(--clr-bg-2);
	}

	.drag-region {
		width: 100%;
		min-height: 100vh;
	}

	.content {
		padding: 48px 32px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		max-width: 640px;
		width: 100%;
		margin: auto;
	}

	.title {
		color: var(--clr-scale-ntrl-0);
		align-self: flex-start;
	}
</style>
