import { invoke } from './ipc';
import type { ForgeName } from '$lib/forge/interface/types';

export class TemplateService {
	constructor(private projectId: string) {}

	async getAvailable(forgeName: ForgeName): Promise<string[]> {
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
