<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import ScrollableContainer, {
		type ScrollableProps
	} from '@gitbutler/ui/scroll/ScrollableContainer.svelte';

	let {
		viewport = $bindable(),
		viewportHeight = $bindable(),
		...restProps
	}: ScrollableProps = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	let scroller: ScrollableContainer;
</script>

<ScrollableContainer
	bind:this={scroller}
	bind:viewport
	bind:viewportHeight
	whenToShow={$userSettings.scrollbarVisibilityState}
	{...restProps}
/>
