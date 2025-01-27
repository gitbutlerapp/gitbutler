<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);
	const shortcutService = getContext(ShortcutService);

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

	shortcutService.on('zoom-in', () => {
		updateZoom(zoom + ZOOM_STEP);
	});
	shortcutService.on('zoom-out', () => {
		updateZoom(zoom - ZOOM_STEP);
	});
	shortcutService.on('zoom-reset', () => {
		updateZoom(DEFAULT_ZOOM);
	});

	onMount(() => {
		if (zoom !== DEFAULT_ZOOM) {
			setDomZoom(zoom);
		}
	});
</script>
