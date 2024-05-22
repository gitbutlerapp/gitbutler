<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';

	export let position: 'top' | 'middle' | 'bottom';

	let expanded = false;
</script>

<SectionCard
	roundedTop={position == 'top'}
	roundedBottom={position == 'bottom'}
	clickable
	on:click={() => (expanded = !expanded)}
>
	<svelte:fragment slot="title">
		<slot name="header"></slot>
	</svelte:fragment>

	{#if expanded}
		<slot></slot>
	{/if}
</SectionCard>
