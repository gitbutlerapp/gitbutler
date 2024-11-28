<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';

	interface Props {
		roundedTop?: boolean;
		roundedBottom?: boolean;
		expanded?: boolean;
		header?: import('svelte').Snippet<[any]>;
		children?: import('svelte').Snippet;
		actions?: import('svelte').Snippet;
	}

	let {
		roundedTop = true,
		roundedBottom = true,
		expanded = $bindable(false),
		header,
		children,
		actions
	}: Props = $props();

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
	{#snippet title()}
		{@render header?.({ expanded })}
	{/snippet}
</SectionCard>

{#if expanded}
	<SectionCard roundedTop={false} {roundedBottom} topDivider>
		{@render children?.()}
	</SectionCard>

	<SectionCard roundedTop={false} {roundedBottom} topDivider>
		{@render actions?.()}
	</SectionCard>
{/if}
