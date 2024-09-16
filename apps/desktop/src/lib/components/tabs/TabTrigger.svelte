<script lang="ts">
	import { getContext, type Snippet } from 'svelte';
	import type { TabContext } from './types';

	interface Props {
		children: Snippet<[{ isActive: boolean; setActive: () => void }]>;
		value: string;
	}

	const { value, children }: Props = $props();

	const tabStore = getContext<TabContext>('tab');
	const selectedIndex = $derived(tabStore.selectedIndex);
	const isActive = $derived($selectedIndex === value);

	function setActive() {
		tabStore?.setSelected(value);
	}
</script>

{@render children({ setActive, isActive })}
