<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';

	export let roundedTop = true;
	export let roundedBottom = true;
	export let expanded = false;

	function maybeToggle() {
		expanded = !expanded;
	}
</script>

<SectionCard
	{roundedTop}
	roundedBottom={roundedBottom && !expanded}
	bottomBorder={!expanded}
	clickable
	on:click={maybeToggle}
>
	<svelte:fragment slot="title">
		<slot name="header" {expanded}></slot>
	</svelte:fragment>
</SectionCard>

{#if expanded}
	<SectionCard roundedTop={false} {roundedBottom} topDivider>
		<slot></slot>
	</SectionCard>

	<SectionCard roundedTop={false} {roundedBottom} topDivider>
		<slot name="actions"></slot>
	</SectionCard>
{/if}
