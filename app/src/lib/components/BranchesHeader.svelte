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
			style="ghost"
			kind="solid"
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
		color: var(--clr-scale-ntrl-0);
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: var(--size-14) var(--size-14) var(--size-12) var(--size-14);
		gap: var(--size-8);
		border-bottom: 1px solid transparent;
		transition: border-bottom var(--transition-fast);
		position: relative;
	}
	.header__filter-btn {
		position: relative;
	}
	.filter-popup-menu {
		position: absolute;
		top: calc(var(--size-control-button) + var(--size-4));
		right: 0;
		z-index: var(--z-floating);
		min-width: 10rem;
	}
	.branches-title {
		display: flex;
		align-items: center;
		gap: var(--size-4);
	}
</style>
