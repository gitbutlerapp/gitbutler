import { invoke } from '@tauri-apps/api/core';
import { readable } from 'svelte/store';

export const editor = readable<string>('vscode', (set) => {
	invoke<string>('get_editor_link_scheme').then((editor) => set(editor));
});
