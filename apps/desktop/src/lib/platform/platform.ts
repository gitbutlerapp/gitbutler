import { platform } from '@tauri-apps/plugin-os';

export const platformName = getPlatform();

function getPlatform() {
	if (import.meta.env.VITE_BUILD_TARGET === 'web') {
		return 'web';
	} else {
		return platform();
	}
}
