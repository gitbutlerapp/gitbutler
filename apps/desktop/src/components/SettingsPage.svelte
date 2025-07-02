<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		title: string | undefined;
		children: Snippet;
	}

	const { title, children }: Props = $props();
</script>

<section class="content-wrapper">
	<ScrollableContainer>
		<div class="content">
			{#if title}
				<h1 class="title text-head-24">
					{title}
				</h1>
			{/if}
			{@render children()}
		</div>
	</ScrollableContainer>
</section>

<style lang="postcss">
	.content-wrapper {
		flex: 1;
		width: 100%;
		height: 100%;
		background-color: var(--clr-bg-2);
	}

	.content {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 640px;
		min-height: 100vh;
		margin: auto;
		padding: 48px 32px;
		gap: 16px;
	}

	.title {
		align-self: flex-start;
		color: var(--clr-scale-ntrl-0);
	}
</style>
