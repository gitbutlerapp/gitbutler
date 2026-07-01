<script lang="ts">
	import { SHORTCUT_SERVICE } from "$lib/shortcuts/shortcutService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { mergeUnlisten } from "@gitbutler/ui/utils/mergeUnlisten";
	import { onMount } from "svelte";

	const uiState = inject(UI_STATE);
	const shortcutService = inject(SHORTCUT_SERVICE);
	const zoom = uiState.global.zoom;

	const MIN_ZOOM = 0.375;
	const MAX_ZOOM = 3;
	const DEFAULT_ZOOM = 1;
	const ZOOM_STEP = 0.0625;

	function setDomZoom(zoom: number) {
		document.documentElement.style.fontSize = zoom + "rem";
	}

	function updateZoom(newZoom: number) {
		const clamped = Math.min(Math.max(newZoom, MIN_ZOOM), MAX_ZOOM);
		setDomZoom(clamped);
		zoom.set(clamped);
	}

	$effect(() =>
		mergeUnlisten(
			shortcutService.on("zoom-in", () => {
				updateZoom(zoom.current + ZOOM_STEP);
			}),
			shortcutService.on("zoom-out", () => {
				updateZoom(zoom.current - ZOOM_STEP);
			}),
			shortcutService.on("zoom-reset", () => {
				updateZoom(DEFAULT_ZOOM);
			}),
		),
	);

	onMount(() => {
		const currentZoom = zoom.current;
		if (currentZoom !== DEFAULT_ZOOM) {
			setDomZoom(currentZoom);
		}
	});
</script>
