<script lang="ts">
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';

	const shortcutService = inject(SHORTCUT_SERVICE);
	const uiState = inject(UI_STATE);

	const unassignedSidebarFolded = uiState.global.unassignedSidebarFolded;

	function toggleSidebar() {
		unassignedSidebarFolded.set(!unassignedSidebarFolded.current);
	}

	$effect(() => shortcutService.on('toggle-sidebar', () => toggleSidebar()));
</script>
