import { writeText, readText } from '@tauri-apps/plugin-clipboard-manager';

export async function writeClipboard(text: string) {
	await writeText(text);
}

export async function readClipboard() {
	return await readText();
}
