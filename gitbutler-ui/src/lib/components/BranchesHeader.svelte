<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import Badge from '$lib/components/Badge.svelte';
	import Button from '$lib/components/Button.svelte';

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

<div class="header">
	<div class="branches-title">
		<span class="text-base-14 text-semibold">Branches</span>

		{#if count !== undefined}
			<Badge {count} />
		{/if}
	</div>
	<div class="header__filter-btn" bind:this={filterButton}>
		<Button
			kind="outlined"
			color="neutral"
			icon={filtersActive ? 'filter-applied-small' : 'filter-small'}
			on:mousedown={onFilterClick}
		>
			Filter
		</Button>
		<div
			class="filter-popup-menu"
			use:clickOutside={{ trigger: filterButton, handler: () => (visible = false) }}
		>
			<slot name="context-menu" {visible} />
		</div>
	</div>
</div>

<style lang="postcss">
	.header {
		display: flex;
		color: var(--clr-theme-scale-ntrl-0);
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: var(--space-14) var(--space-14) var(--space-12) var(--space-14);
		gap: var(--space-8);
		border-bottom: 1px solid transparent;
		transition: border-bottom var(--transition-fast);
		position: relative;
	}
	.header__filter-btn {
		position: relative;
	}
	.filter-popup-menu {
		position: absolute;
		top: calc(var(--size-btn-m) + var(--space-4));
		right: 0;
		z-index: 10;
		min-width: 10rem;
	}
	.branches-title {
		display: flex;
		align-items: center;
		gap: var(--space-4);
	}
</style>
