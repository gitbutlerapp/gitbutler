<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		searchValue: string;
		placeholder?: string;
	}

	let { placeholder = 'Search…', searchValue = $bindable() }: Props = $props();

	let inputEl: HTMLInputElement;

	function resetFilter() {
		searchValue = '';
		inputEl.focus();
	}

	function handleInput(event: Event) {
		searchValue = (event.target as HTMLInputElement).value;
	}
</script>

<div class="container">
	{#if !searchValue}
		<i class="icon search-icon">
			<Icon name="search" />
		</i>
	{:else}
		<button type="button" class="icon" onclick={resetFilter}>
			<Icon name="clear-input" />
		</button>
	{/if}

	<input
		bind:this={inputEl}
		class="text-13 search-input"
		type="text"
		{placeholder}
		bind:value={searchValue}
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
		display: flex;
		pointer-events: none;
	}
</style>
