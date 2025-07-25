<script lang="ts">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/shared/context';
	import { ScrollableContainer, type ScrollableProps } from '@gitbutler/ui';

	let {
		viewport = $bindable(),
		viewportHeight = $bindable(),
		...restProps
	}: ScrollableProps = $props();

	const userSettings = inject(SETTINGS);

	let scroller: ScrollableContainer;
</script>

<ScrollableContainer
	bind:this={scroller}
	bind:viewport
	bind:viewportHeight
	whenToShow={$userSettings.scrollbarVisibilityState}
	{...restProps}
/>
