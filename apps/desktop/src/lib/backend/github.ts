import { invoke } from '$lib/backend/ipc';

export type Verification = {
	user_code: string;
	device_code: string;
};

export type PullRequestTemplatePaths = {
	value: string;
	label: string;
};

export async function initDeviceOauth() {
	return await invoke<Verification>('init_device_oauth');
}

export async function checkAuthStatus(params: { deviceCode: string }) {
	return await invoke<string>('check_auth_status', params);
}

export async function getAvailablePullRequestTemplates(
	projectPath: string
): Promise<PullRequestTemplatePaths[] | undefined> {
	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	const path = await import('@tauri-apps/api/path');
	const targetPath = await path.join(projectPath, '.github');

	const availableTemplates: PullRequestTemplatePaths[] | undefined = await invoke(
		'get_available_github_pr_templates',
		{
			path: targetPath
		}
	);

	return availableTemplates;
}
