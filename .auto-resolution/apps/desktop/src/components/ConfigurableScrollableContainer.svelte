<script lang="ts">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import { ScrollableContainer, type ScrollableProps } from '@gitbutler/ui';

	let {
		viewport = $bindable(),
		viewportHeight = $bindable(),
		...restProps
	}: ScrollableProps = $props();

	const userSettings = inject(SETTINGS);
	let scrollableContainer: ScrollableContainer;

	// Export method to scroll to bottom
	export function scrollToBottom() {
		if (scrollableContainer?.scrollToBottom) {
			scrollableContainer.scrollToBottom();
		}
	}

	// Export method to scroll to top
	export function scrollToTop() {
		if (scrollableContainer?.scrollToTop) {
			scrollableContainer.scrollToTop();
		}
	}
</script>

<ScrollableContainer
	bind:this={scrollableContainer}
	bind:viewport
	bind:viewportHeight
	whenToShow={$userSettings.scrollbarVisibilityState}
	{...restProps}
/>
