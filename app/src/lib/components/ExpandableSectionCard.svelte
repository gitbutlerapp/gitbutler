<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';

	export let roundedTop = true;
	export let roundedBottom = true;
	export let expanded = false;
	export let displayActions = false;
	export let disableClosing = false;

	function maybeToggle() {
		if (disableClosing && expanded) return;

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
	<SectionCard
		hasTopRadius={false}
		roundedTop={false}
		roundedBottom={roundedBottom && !displayActions}
		bottomBorder={!displayActions}
		topDivider
	>
		<slot></slot>
	</SectionCard>

	{#if displayActions}
		<SectionCard hasTopRadius={false} roundedTop={false} {roundedBottom} topDivider>
			<slot name="actions"></slot>
		</SectionCard>
	{/if}
{/if}
