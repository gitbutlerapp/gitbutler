<script lang="ts">
	import { type Selectable } from './Select.svelte';
	import Icon from '$lib/shared/Icon.svelte';

	interface Props {
		placeholder?: string;
		items: Selectable<string>[];
		onSort?: (items: Selectable<string>[]) => void;
	}

	const { placeholder = 'Search…', items, onSort }: Props = $props();

	let value = $state('');
	let filteredItems = items;

	let inputEl: HTMLInputElement;

	function handleInput(event: Event) {
		value = (event.target as HTMLInputElement).value;
		onSort?.(items);
	}
</script>

<div class="container">
	{#if !value}
		<i class="icon search-icon">
			<Icon name="search" />
		</i>
	{:else}
		<button
			class="icon"
			onclick={() => {
				value = '';
				inputEl.focus();
			}}
		>
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
	}

	.search-input {
		padding: 12px 34px 12px 12px;
		width: 100%;
		background-color: var(--clr-bg-1);
		color: var(--clr-text-1);

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
	}

	.search-icon {
		pointer-events: none;
	}
</style>
