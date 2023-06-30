<script lang="ts">
	import { theme, setTheme } from '$lib/theme';

	function onThemeChange(e: Event & { currentTarget: HTMLSelectElement }) {
		setTheme(e.currentTarget.value);
	}
</script>

<div>
	<select bind:value={$theme} on:change={onThemeChange}>
		<option value="system">System preference</option>
		<option value="light">Light</option>
		<option value="dark">Dark</option>
	</select>
</div>
