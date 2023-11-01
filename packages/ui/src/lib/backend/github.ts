import { invoke } from '$lib/backend/ipc';

export type Verification = {
	user_code: string;
	device_code: string;
};

export function initDeviceOauth() {
	return invoke<Verification>('init_device_oauth');
}

export function checkAuthStatus(params: { deviceCode: string }) {
	return invoke<string>('check_auth_status', params);
}
