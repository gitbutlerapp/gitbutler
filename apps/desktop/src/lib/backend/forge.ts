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

	async getReviewTemplateContent(templatePath: string): Promise<string> {
		const fileContents: string = await invoke('get_review_template_contents', {
			relativePath: templatePath,
			projectId: this.projectId
		});

		return fileContents;
	}
}
