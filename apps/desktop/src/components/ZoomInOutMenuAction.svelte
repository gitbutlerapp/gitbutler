<script lang="ts">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { inject } from '@gitbutler/core/context';
	import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
	import { onMount } from 'svelte';

	const userSettings = inject(SETTINGS);
	const shortcutService = inject(SHORTCUT_SERVICE);

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

	$effect(() =>
		mergeUnlisten(
			shortcutService.on('zoom-in', () => {
				updateZoom(zoom + ZOOM_STEP);
			}),
			shortcutService.on('zoom-out', () => {
				updateZoom(zoom - ZOOM_STEP);
			}),
			shortcutService.on('zoom-reset', () => {
				updateZoom(DEFAULT_ZOOM);
			})
		)
	);

	onMount(() => {
		if (zoom !== DEFAULT_ZOOM) {
			setDomZoom(zoom);
		}
	});
</script>
