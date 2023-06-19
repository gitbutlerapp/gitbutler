<script lang="ts">
	import { appWindow, type Theme } from '@tauri-apps/api/window';

	const themeStorageKey = 'theme';
	let systemTheme: string | null;
	let selectedTheme: string | null;

	appWindow.theme().then((value: Theme | null) => {
		selectedTheme = localStorage.getItem(themeStorageKey);
		systemTheme = value;
		updateDom();
	});
	appWindow.onThemeChanged((e) => {
		systemTheme = e.payload;
		updateDom();
	});

	function onThemeChange(e: Event & { currentTarget: HTMLSelectElement }) {
		selectedTheme = e.currentTarget.value;
		localStorage.setItem(themeStorageKey, selectedTheme);
		updateDom();
	}

	function updateDom() {
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
	<select bind:value={selectedTheme} on:change={onThemeChange}>
		<option>system</option>
		<option>light</option>
		<option>dark</option>
	</select>
</div>
