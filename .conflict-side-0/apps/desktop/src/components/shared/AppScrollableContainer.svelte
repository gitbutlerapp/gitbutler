<script lang="ts">
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ScrollableContainer, type ScrollableProps } from "@gitbutler/ui";

	let {
		viewport = $bindable(),
		scrollableEl = $bindable(),
		viewportHeight = $bindable(),
		...restProps
	}: ScrollableProps = $props();

	const uiState = inject(UI_STATE);
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
	bind:scrollableEl
	bind:viewportHeight
	whenToShow={uiState.global.scrollbarVisibilityState.current}
	{...restProps}
/>
