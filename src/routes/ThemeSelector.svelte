<script lang="ts">
	import { appWindow, type Theme } from '@tauri-apps/api/window';

	const themeStorageKey = 'theme';
	let systemTheme: string | null;
	let selectedTheme: string | null;

	appWindow.theme().then((value: Theme | null) => {
		systemTheme = value;
		selectedTheme = localStorage.getItem(themeStorageKey);
		updateDomTheme();
	});
	appWindow.onThemeChanged((e) => {
		console.log(e);
		systemTheme = e.payload;
		updateDomTheme();
	});

	function onDarkModeChange(e: Event) {
		selectedTheme = (e.target as HTMLSelectElement).value;
		localStorage.setItem(themeStorageKey, selectedTheme);
		updateDomTheme();
	}

	function updateDomTheme() {
		const docEl = document.documentElement;
		if (selectedTheme == 'dark' || (selectedTheme == 'system' && systemTheme == 'dark')) {
			docEl.classList.add('dark');
			docEl.style.colorScheme = 'dark';
		} else if (selectedTheme == 'light' || (selectedTheme == 'system' && systemTheme == 'light')) {
			docEl.classList.remove('dark');
			docEl.style.colorScheme = 'light';
		}
	}
</script>

<div>
	<label for="dark-mode-toggle" class="mr-2">Theme</label>
	<select bind:value={selectedTheme} on:change={onDarkModeChange}>
		<option>system</option>
		<option>light</option>
		<option>dark</option>
	</select>
</div>
