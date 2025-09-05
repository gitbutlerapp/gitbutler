<script lang="ts">
	import Icon from '$components/Icon.svelte';

	interface Props {
		searchValue: string;
		placeholder?: string;
	}

	let { placeholder = 'Search…', searchValue = $bindable() }: Props = $props();

	let inputEl: HTMLInputElement;

	export function focus() {
		inputEl?.focus();
	}

	function resetFilter() {
		searchValue = '';
		inputEl.focus();
	}

	function handleInput(event: Event) {
		searchValue = (event.target as HTMLInputElement).value;
	}

	function handleKeyDown(event: KeyboardEvent) {
		const { key } = event;

		// Allow arrow keys, Enter, and Escape to bubble up to the parent Select
		// for dropdown navigation while keeping the search input focused
		if (key === 'ArrowUp' || key === 'ArrowDown' || key === 'Enter' || key === 'Escape') {
			// Don't prevent default for arrow keys so cursor behavior in input is preserved
			// but don't stop propagation so parent can handle navigation
			return;
		}

		// Stop other keys from bubbling up to prevent interference
		event.stopPropagation();
	}
</script>

<div class="container">
	{#if !searchValue}
		<i class="icon search-icon">
			<Icon name="search" />
		</i>
	{:else}
		<button type="button" class="icon" onclick={resetFilter}>
			<Icon name="cross-in-circle" />
		</button>
	{/if}

	<input
		bind:this={inputEl}
		class="text-13 search-input"
		type="text"
		{placeholder}
		bind:value={searchValue}
		oninput={handleInput}
		onkeydown={handleKeyDown}
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
		width: 100%;
		padding: 12px 34px 12px 12px;

		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		color: var(--clr-text-1);
		transition: border-color var(--transition-fast);

		&:hover,
		&:focus-within {
			outline: none;
			background-color: var(--clr-bg-1-muted);
		}

		&::placeholder {
			color: var(--clr-text-3);
		}
	}

	.icon {
		display: flex;
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
		display: flex;
		pointer-events: none;
	}
</style>
