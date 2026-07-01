<script lang="ts">
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Scrollbar } from "@gitbutler/ui";

	interface Props {
		viewport: HTMLDivElement;
		initiallyVisible?: boolean;
		thickness?: string;
		shift?: string;
		horz?: boolean;
		zIndex?: string;
		onthumbdrag?: (dragging: boolean) => void;
		onscroll?: (e: Event) => void;
		updateTrack?: () => void;
	}

	const {
		viewport,
		initiallyVisible = false,
		thickness = "0.563rem",
		shift = "0",
		horz = false,
		zIndex = "var(--z-lifted)",
		onthumbdrag,
		onscroll,
	}: Props = $props();

	const uiState = inject(UI_STATE);

	let scrollbar = $state<Scrollbar>();

	export function updateTrack() {
		scrollbar?.updateTrack();
	}
</script>

<Scrollbar
	{viewport}
	{initiallyVisible}
	{thickness}
	{shift}
	{horz}
	{zIndex}
	{onthumbdrag}
	{onscroll}
	whenToShow={uiState.global.scrollbarVisibilityState.current}
/>
