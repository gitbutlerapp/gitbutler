export class DesktopRoutesService {
	constructor(private readonly projectId: string) {}

	get projectPath() {
		return `/${this.projectId}`;
	}
	get settingsPath() {
		return `/${this.projectId}/settings`;
	}
	get workspacePath() {
		return `/${this.projectId}/workspace`;
	}
	get branchesPath() {
		return `/${this.projectId}/branches`;
	}
	get targetPath() {
		return `/${this.projectId}/target`;
	}
	get historyPath() {
		return `/${this.projectId}/history`;
	}
}
