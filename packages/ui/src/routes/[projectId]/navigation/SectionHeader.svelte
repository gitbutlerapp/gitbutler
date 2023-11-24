<script lang="ts">
	import Badge from '$lib/components/Badge.svelte';
	import Icon from '$lib/icons/Icon.svelte';

	export let scrolled: boolean;
	export let count: number | undefined;
	export let expanded = true;
	export let expandable = false;
</script>

<button
	on:click={() => {
		if (expandable) expanded = !expanded;
	}}
	disabled={count && count > 0 ? false : true}
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
	{#if expandable && count}
		<Icon name={expanded ? 'chevron-down-16' : 'chevron-top-16'} />
	{/if}
</button>

<style lang="postcss">
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
	.header:disabled {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
