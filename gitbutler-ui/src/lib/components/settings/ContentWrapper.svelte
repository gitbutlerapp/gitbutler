<script lang="ts">
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	export let title: string | undefined = undefined;
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
				<slot />
			</div>
		</div>
	</ScrollableContainer>
</section>

<style lang="post-css">
	.content-wrapper {
		user-select: none;
		width: 100%;
		height: 100%;
		flex: 1;
		background-color: var(--clr-theme-container-pale);
	}

	.drag-region {
		width: 100%;
		min-height: 100vh;
	}

	.content {
		padding: var(--size-48) var(--size-32);
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
		max-width: 40rem;
		width: 100%;
		margin: auto;
	}

	.title {
		color: var(--clr-theme-scale-ntrl-0);
		align-self: flex-start;
	}
</style>
