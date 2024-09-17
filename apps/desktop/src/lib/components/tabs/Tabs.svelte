<script lang="ts">
	import { type TabContext } from './types';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
		defaultSelected: string;
	}

	const { children, defaultSelected }: Props = $props();

	let selectedIndex = writable(defaultSelected);

	const context: TabContext = {
		selectedIndex,
		setSelected: (i) => {
			selectedIndex.set(i);
			return selectedIndex;
		}
	};

	setContext<TabContext>('tab', context);
</script>

<section class="tab-wrapper">
	{@render children()}
</section>

<style>
	.tab-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		margin: 0 auto;
	}
</style>
