import { invoke } from '../backend/ipc';

export class TemplateService {
	constructor(private projectId: string) {}

	async getAvailable(forgeName: string): Promise<string[]> {
		return await invoke<string[]>('get_available_review_templates', {
			projectId: this.projectId,
			forge: { name: forgeName }
		});
	}

	async getContent(forgeName: string, templatePath: string): Promise<string> {
		return await invoke('get_review_template_contents', {
			relativePath: templatePath,
			projectId: this.projectId,
			forge: { name: forgeName }
		});
	}
}
