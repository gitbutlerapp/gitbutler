import { IS_TAURI_ENV } from '$lib/backend/tauri';
import { platform } from '@tauri-apps/plugin-os';

export const platformName = IS_TAURI_ENV ? platform() : undefined;
