<script lang="ts">
	import { type SelectItem } from './Select.svelte';
	import Icon from '$lib/shared/Icon.svelte';

	interface Props {
		placeholder?: string;
		items: SelectItem[];
		onSort?: (items: SelectItem[]) => void;
	}

	const { placeholder = 'Search…', items, onSort }: Props = $props();

	let value = $state('');
	let filteredItems = $state(items);

	let inputEl: HTMLInputElement;

	function handleFilter() {
		filteredItems = items.filter((item) => item.label.toLowerCase().includes(value.toLowerCase()));
	}

	function resetFilter() {
		value = '';
		handleFilter();
		inputEl.focus();
	}

	function handleInput(event: Event) {
		value = (event.target as HTMLInputElement).value;
		handleFilter();
	}

	$effect(() => {
		onSort?.(filteredItems);
	});
</script>

<div class="container">
	{#if !value}
		<i class="icon search-icon">
			<Icon name="search" />
		</i>
	{:else}
		<button class="icon" onclick={resetFilter}>
			<Icon name="clear-input" />
		</button>
	{/if}

	<input
		bind:this={inputEl}
		class="text-base-13 search-input"
		type="text"
		{placeholder}
		bind:value
		oninput={handleInput}
		autocorrect="off"
		autocomplete="off"
	/>
</div>

<style lang="postcss">
	.container {
		position: relative;
		user-select: text;
	}

	.search-input {
		padding: 12px 34px 12px 12px;
		width: 100%;
		background-color: var(--clr-bg-1);
		color: var(--clr-text-1);

		border-bottom: 1px solid var(--clr-border-2);
		transition: border-color var(--transition-fast);

		&:hover,
		&:focus-within {
			background-color: var(--clr-bg-1-muted);
			outline: none;
		}

		&::placeholder {
			color: var(--clr-text-3);
		}
	}

	.icon {
		position: absolute;
		top: 50%;
		right: 12px;
		transform: translateY(-50%);
		color: var(--clr-scale-ntrl-50);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-scale-ntrl-40);
		}
	}

	.search-icon {
		pointer-events: none;
	}
</style>
