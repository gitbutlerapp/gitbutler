<script lang="ts">
	import { getContext, type Snippet } from 'svelte';
	import type { TabContext } from './types';

	interface Props {
		children: Snippet;
		value: string;
		disabled?: boolean;
	}

	const { value, children, disabled }: Props = $props();

	const tabStore = getContext<TabContext>('tab');
	const selectedIndex = $derived(tabStore.selectedIndex);
	const isActive = $derived($selectedIndex === value);

	function setActive() {
		tabStore?.setSelected(value);
	}
</script>

<button
	{value}
	class="tab-trigger text-12"
	{disabled}
	onclick={setActive}
	class:disabled
	class:active={isActive}
>
	{@render children()}
</button>

<style>
	.tab-trigger {
		width: auto;
		height: 28px;
		flex-grow: 1;
		user-select: none;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		color: var(--btn-text-clr);
		background: var(--btn-bg);
		border: 1px solid transparent;
		transition:
			background var(--transition-fast),
			color var(--transition-fast);

		&:disabled {
			cursor: default;
			opacity: 0.5;
		}

		&.active {
			--btn-text-clr: var(--clr-theme-ntrl-on-element);
			--btn-bg: var(--clr-theme-ntrl-element);
		}

		&:not(:last-child) {
			border-right: 1px solid var(--clr-border-2);
		}

		&:first-child {
			border-top-left-radius: var(--radius-m);
			border-bottom-left-radius: var(--radius-m);
		}

		&:last-child {
			border-top-right-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
		}
	}
</style>
