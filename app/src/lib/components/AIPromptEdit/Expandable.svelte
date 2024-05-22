<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';

	export let roundedTop = true;
	export let roundedBottom = true;

	let expanded = false;
</script>

<SectionCard {roundedTop} {roundedBottom} clickable on:click={() => (expanded = !expanded)}>
	<svelte:fragment slot="title">
		<slot name="header"></slot>
	</svelte:fragment>

	{#if expanded}
		<slot></slot>
	{/if}
</SectionCard>
