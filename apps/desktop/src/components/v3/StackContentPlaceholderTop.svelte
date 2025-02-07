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
	<div class="placeholder__svg" in:fade={{ duration: 250 }}>
		{@render illustration()}
	</div>
{/if}
<div class="placeholder__text">
	{#if title}
		<div class="text-18 text-semibold text-clr1">{@render title()}</div>
	{/if}
	{#if body}
		<div class="text-13 text-clr2">{@render body()}</div>
	{/if}
</div>

<style>
	.placeholder__svg {
		width: 300px;
	}

	.placeholder__text {
		text-align: left;
		display: inline;

		white-space: pre-wrap;
		text-wrap: wrap;
	}

	p {
		display: inline-block;
		line-height: 1.75;
	}
</style>
