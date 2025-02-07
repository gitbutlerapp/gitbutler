<script lang="ts">
	import { fade } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	interface Props {
		illustration?: Snippet;
		title?: Snippet;
		body?: Snippet;
	}

	const { illustration, title, body }: Props = $props();
</script>

{#if illustration}
	<div class="illustration" in:fade={{ duration: 250 }}>
		{@render illustration()}
	</div>
{/if}
<div class="text">
	{#if title}
		<div class="text-18 text-semibold text-clr1" class:center={!body}>
			{@render title()}
		</div>
	{/if}
	{#if body}
		<p class="text-13 text-clr2">
			{@render body()}
		</p>
	{/if}
</div>

<style>
	.illustration {
		width: 300px;
	}

	.text {
		display: flex;
		max-width: 375px;
		flex-direction: column;
		gap: 8px;
		text-align: left;

		& p {
			line-height: 1.75;
		}

		.center {
			text-align: center;
			max-width: 300px;
			font-size: 13px;
			font-weight: 300;
			color: var(--clr-text-2);
		}
	}
</style>
