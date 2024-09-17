<script lang="ts">
	import { type TabContext } from './types';
	import { getContext, type Snippet } from 'svelte';

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
	role="tab"
	tabindex={isActive ? -1 : 0}
	aria-selected={isActive}
	id={value}
	{value}
	{disabled}
	onclick={setActive}
	class="segment-control-item"
	class:disabled
	class:active={isActive}
>
	<span class="text-12 segment-control-item__label">
		{@render children()}
	</span>
</button>
