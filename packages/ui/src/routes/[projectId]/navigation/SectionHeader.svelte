<script lang="ts">
	import Badge from '$lib/components/Badge.svelte';
	import IconTriangleDown from '$lib/icons/IconTriangleDown.svelte';

	export let scrolled: boolean;
	export let count: string | number | undefined;
	export let expanded = true;
	export let expandable = false;
</script>

<button
	on:click={() => {
		if (expandable) expanded = !expanded;
	}}
	class="header border-t p-4 font-bold"
	style:border-color="var(--border-surface)"
	class:border-b={scrolled}
>
	<div class="whitespace-nowrap font-bold">
		<slot />
		{#if count !== undefined}
			<Badge {count} />
		{/if}
	</div>
	{#if expandable}
		<IconTriangleDown class={expanded ? '-rotate-180' : ''} />
	{/if}
</button>

<style>
	.header {
		display: flex;
		color: var(--clr-theme-scale-ntrl-50);
		align-items: center;
		justify-content: space-between;
		width: 100%;
	}
	.header:hover,
	.header:focus {
		color: var(--clr-theme-scale-ntrl-40);
	}
</style>
