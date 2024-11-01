<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		roundedTop?: boolean;
		roundedBottom?: boolean;
		expanded?: boolean;
		header?: Snippet<[any]>;
		children?: Snippet;
		actions?: Snippet;
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
	<svelte:fragment slot="title">
		{@render header?.({ expanded })}
	</svelte:fragment>
</SectionCard>

{#if expanded}
	<SectionCard roundedTop={false} {roundedBottom} topDivider>
		{@render children?.()}
	</SectionCard>

	<SectionCard roundedTop={false} {roundedBottom} topDivider>
		{@render actions?.()}
	</SectionCard>
{/if}
