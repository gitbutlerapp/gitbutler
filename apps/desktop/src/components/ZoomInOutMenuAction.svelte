<script lang="ts">
	import { listen } from '$lib/backend/ipc';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	let zoom = $state($userSettings.zoom);

	const MIN_ZOOM = 0.375;
	const MAX_ZOOM = 3;
	const DEFAULT_ZOOM = 1;
	const ZOOM_STEP = 0.0625;

	function setDomZoom(zoom: number) {
		document.documentElement.style.fontSize = zoom + 'rem';
	}

	function updateZoom(newZoom: number) {
		zoom = Math.min(Math.max(newZoom, MIN_ZOOM), MAX_ZOOM);
		setDomZoom(zoom);
		userSettings.update((s) => ({ ...s, zoom }));
	}

	const handleKeyDown = createKeybind({
		'$mod++': () => updateZoom(zoom + ZOOM_STEP),
		'$mod+=': () => updateZoom(zoom + ZOOM_STEP),
		'$mod+-': () => updateZoom(zoom - ZOOM_STEP),
		'$mod+0': () => updateZoom(DEFAULT_ZOOM)
	});

	onMount(() => {
		if (zoom !== DEFAULT_ZOOM) {
			setDomZoom(zoom);
		}

		const unsubscribeZoomIn = listen<string>('menu://view/zoom-in/clicked', () =>
			updateZoom(zoom + ZOOM_STEP)
		);
		const unsubscribeZoomOut = listen<string>('menu://view/zoom-out/clicked', () =>
			updateZoom(zoom - ZOOM_STEP)
		);
		const unsubscribeResetZoom = listen<string>('menu://view/zoom-reset/clicked', () =>
			updateZoom(DEFAULT_ZOOM)
		);

		return () => {
			unsubscribeZoomIn();
			unsubscribeZoomOut();
			unsubscribeResetZoom();
		};
	});
</script>

<svelte:window onkeydown={handleKeyDown} />
