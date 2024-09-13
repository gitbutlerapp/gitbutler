<script lang="ts">
	import { onMount, setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { TabContext } from './types';
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet<[{ tabName: string }]>;
		name: string;
		defaultSelected: string;
	}

	const { children, name, defaultSelected }: Props = $props();
	console.log('Tabs.name', name);

	let selectedIndex = writable('');

	const context: TabContext = {
		selectedIndex,
		setSelected: (i) => {
			console.log('SELECTING ', i);
			selectedIndex.set(i);
			return selectedIndex;
		}
	};

	const tabs = setContext<TabContext>('tab', context);

	onMount(() => {
		tabs?.setSelected(defaultSelected);
	});
</script>

<section class="tab-wrapper">
	{@render children({ tabName: name })}
</section>

<style>
	.tab-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		margin: 0 auto;
	}
</style>
