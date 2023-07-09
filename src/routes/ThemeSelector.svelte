<script lang="ts">
	import type { SettingsStore } from '$lib/userSettings';

	export let userSettings: SettingsStore;
</script>

<div>
	<select
		bind:value={$userSettings.theme}
		on:change={(e) => userSettings.update((s) => ({ ...s, theme: e.currentTarget.value }))}
	>
		<option value="system">System preference</option>
		<option value="light">Light</option>
		<option value="dark">Dark</option>
	</select>
</div>
