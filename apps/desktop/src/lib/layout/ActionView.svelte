<script lang="ts">
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<ScrollableContainer wide>
	<div class="wrapper">
		<div class="content">
			{#if children}
				{@render children()}
			{/if}
		</div>
	</div>
</ScrollableContainer>

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;

		flex-grow: 1;

		align-items: center;
		justify-content: center;

		background-color: var(--clr-bg-1);
	}

	.content {
		width: 100%;
		max-width: 560px;
		padding: 48px 32px;
	}
</style>
