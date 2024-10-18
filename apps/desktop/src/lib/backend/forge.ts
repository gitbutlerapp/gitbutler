import { invoke } from './ipc';

export type ForgeType = 'github' | 'gitlab' | 'bitbucket' | 'azure';

export class ForgeService {
	constructor(private projectId: string) {}

	async getAvailableReviewTemplates(): Promise<string[]> {
		const templates = await invoke<string[]>('get_available_review_templates', {
			projectId: this.projectId
		});

		return templates;
	}
}
