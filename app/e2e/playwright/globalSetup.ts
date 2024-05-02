import { type FullConfig } from '@playwright/test';
import { mockIPC } from '@tauri-apps/api/mocks';
import { mockWindows } from '@tauri-apps/api/mocks';
// import { invoke } from '@tauri-apps/api/tauri';

async function globalSetup(config: FullConfig) {
	// Object.defineProperty(window, 'crypto', {
	// 	value: {
	// 		// @ts-ignore
	// 		getRandomValues: (buffer) => {
	// 			return randomFillSync(buffer);
	// 		}
	// 	}
	// });

	// mockWindows('main');
	mockIPC((cmd, args) => {
		// simulated rust command called "add" that just adds two numbers
		if (cmd === 'add') {
			return (args.a as number) + (args.b as number);
		}
	});
}

export default globalSetup;
