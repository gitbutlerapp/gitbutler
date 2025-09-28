<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		buttons?: Snippet;
		children: Snippet;
	}

	const { children, buttons }: Props = $props();
</script>

<div class="section-header">
	<h2 class="section-header__title">
		{@render children()}
	</h2>

	{#if buttons}
		<div class="section-header__buttons">
			{@render buttons()}
		</div>
	{/if}
</div>

<style>
	.section-header {
		display: flex;
		grid-column: narrow-start / narrow-end;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 30px;
	}

	.section-header__title {
		font-size: 60px;
		line-height: 1.1;
		font-family: var(--fontfamily-accent);
	}

	.section-header__buttons {
		display: flex;
		gap: 6px;
	}
</style>
