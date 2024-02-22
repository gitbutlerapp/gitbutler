<script lang="ts">
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	export let title: string | undefined = undefined;
</script>

<section class="content-wrapper" data-tauri-drag-region>
	<ScrollableContainer>
		<div class="content">
			{#if title}
				<h1 class="title text-head-24">
					{title}
				</h1>
			{/if}
			<slot />
		</div>
	</ScrollableContainer>
</section>

<style lang="post-css">
	.content-wrapper {
		user-select: none;
		width: 100%;
		flex: 1;
		background-color: var(--clr-theme-container-light);
		padding: var(--space-48) var(--space-32);
	}

	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
		max-width: 37rem;
		width: 100%;
		margin: auto;
	}

	.title {
		color: var(--clr-theme-scale-ntrl-0);
	}
</style>
