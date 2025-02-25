import {
	isPermissionGranted,
	requestPermission,
	sendNotification
} from '@tauri-apps/plugin-notification';

async function checkPermission() {
	const permission = await isPermissionGranted();
	if (!permission) {
		const requestResponse = await requestPermission();
		return requestResponse === 'granted';
	}
	return true;
}

export async function sendOSNotification(title: string, body: string) {
	const canSendNotification = await checkPermission();
	if (!canSendNotification) {
		// Not an error, but maybe in the future we can log this?
		return;
	}

	sendNotification({ title, body });
}
