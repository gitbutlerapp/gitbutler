<script lang="ts">
	import { getContext, type Snippet } from 'svelte';
	import type { TabContext } from './types';

	interface Props {
		children: Snippet;
		value: string;
		label?: string;
		disabled?: boolean;
	}

	const { value, label, children, disabled }: Props = $props();

	const tabStore = getContext<TabContext>('tab');
	const selectedIndex = $derived(tabStore.selectedIndex);
	const isActive = $derived($selectedIndex === value);

	function setActive() {
		tabStore?.setSelected(value);
	}
</script>

<button {value} class="tab-trigger" onclick={setActive} class:disabled class:active={isActive}>
	{#if label}
		<span class="label">{label}</span>
	{:else}
		{@render children()}
	{/if}
</button>

<style>
	.tab-trigger {
		min-width: 64px;
		width: auto;
		height: 32px;
		padding: 16px;
		user-select: none;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-m);
		border: 1px solid transparent;
		cursor: pointer;
		color: var(--btn-text-clr);
		background: var(--btn-bg);

		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);

		&:disabled {
			cursor: default;
			opacity: 0.5;
		}

		&.active {
			border-color: var(--clr-theme-pop-element);
			background-color: var(--clr-bg-1);
		}

		.label {
			display: inline-flex;
			white-space: nowrap;
		}
	}
</style>
