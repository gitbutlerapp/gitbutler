<script lang="ts">
	import { slugify } from '@gitbutler/ui/utils/string';

	interface Props {
		depth: number;
		raw: string;
		text: string;
	}

	const { depth, raw, text }: Props = $props();

	const id = $derived(slugify(text) ?? text);
</script>

{#if depth === 1}
	<h1 {id}>
		{text}
	</h1>
{:else if depth === 2}
	<h2 {id}>
		{text}
	</h2>
{:else if depth === 3}
	<h3 {id}>
		{text}
	</h3>
{:else if depth === 4}
	<h4 {id}>
		{text}
	</h4>
{:else if depth === 5}
	<h5 {id}>
		{text}
	</h5>
{:else if depth === 6}
	<h6 {id}>
		{text}
	</h6>
{:else}
	{raw}
{/if}
