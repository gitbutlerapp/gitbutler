import { invoke } from '$lib/backend/ipc';

export type Verification = {
	user_code: string;
	device_code: string;
};

export async function initDeviceOauth() {
	return await invoke<Verification>('init_device_oauth');
}

export async function checkAuthStatus(params: { deviceCode: string }) {
	return await invoke<string>('check_auth_status', params);
}
