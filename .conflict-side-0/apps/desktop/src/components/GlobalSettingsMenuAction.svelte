<script lang="ts">
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { inject } from '@gitbutler/core/context';

	const shortcutService = inject(SHORTCUT_SERVICE);
	const { openGeneralSettings } = useSettingsModal();

	$effect(() =>
		shortcutService.on('global-settings', () => {
			openGeneralSettings();
		})
	);
</script>
