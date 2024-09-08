import { invoke } from '$lib/backend/ipc';
import { Project } from '$lib/backend/projects';
import { plainToInstance } from 'class-transformer';

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
	projectId: string
): Promise<PullRequestTemplatePaths[] | undefined> {
	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	const path = await import('@tauri-apps/api/path');
	const currentProject = plainToInstance(Project, await invoke('get_project', { id: projectId }));
	const targetPath = await path.join(currentProject.path, '.github');

	const availableTemplates: PullRequestTemplatePaths[] | undefined = await invoke(
		'get_available_pull_request_templates',
		{
			path: targetPath
		}
	);
	console.log('getAvailablePullRequestTemplates.templates', availableTemplates);

	return availableTemplates;
}
