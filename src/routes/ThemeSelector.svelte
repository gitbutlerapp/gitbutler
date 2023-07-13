<script lang="ts">
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { getContext } from 'svelte';

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
</script>

<div>
	<select
		value={$userSettings.theme}
		on:change={(e) => userSettings.update((s) => ({ ...s, theme: e.currentTarget.value }))}
	>
		<option value="system">System preference</option>
		<option value="light">Light</option>
		<option value="dark">Dark</option>
	</select>
</div>
