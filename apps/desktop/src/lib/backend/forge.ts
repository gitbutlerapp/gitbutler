import { invoke } from './ipc';

export type ForgeType = 'github' | 'gitlab' | 'bitbucket' | 'azure';

export async function getAvailableReviewTemplates(projectId: string): Promise<string[]> {
	const templates = await invoke<string[]>('get_available_review_templates', {
		projectId
	});

	return templates;
}
