<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import Badge from '$lib/components/Badge.svelte';
	import Button from '$lib/components/Button.svelte';

	export let scrolled: boolean;
	export let count: number | undefined;
	export let filtersActive = false;

	let visible = false;
	let filterButton: HTMLDivElement;

	function onFilterClick(e: Event) {
		visible = !visible;
		e.preventDefault();
		e.stopPropagation();
	}
</script>

<div class="header" class:header-scrolled={scrolled}>
	<div class="branches-title">
		<span class="text-base-13 text-semibold">Branches</span>

		{#if count !== undefined}
			<Badge {count} />
		{/if}
	</div>
	<div bind:this={filterButton}>
		<Button
			kind="outlined"
			color="neutral"
			icon={filtersActive ? 'filter-applied-small' : 'filter-small'}
			on:click={onFilterClick}
		>
			Filter
		</Button>
	</div>
	<div
		class="filter-popup-menu"
		use:clickOutside={{ trigger: filterButton, handler: () => (visible = false) }}
	>
		<slot name="context-menu" {visible} />
	</div>
</div>

<style lang="postcss">
	.header {
		display: flex;
		color: var(--clr-theme-scale-ntrl-40);
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: var(--space-16) var(--space-12);
		gap: var(--space-8);
		border-top: 1px solid var(--clr-theme-container-outline-light);
		border-bottom: 1px solid transparent;
		transition: border-bottom var(--transition-fast);
		position: relative;
	}
	.header-scrolled {
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}
	.filter-popup-menu {
		position: absolute;
		top: var(--space-48);
		right: var(--space-12);
		z-index: 10;
		min-width: 10rem;
	}
	.branches-title {
		display: flex;
		align-items: center;
		gap: var(--space-4);
	}
</style>
