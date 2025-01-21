export class DesktopRoutesService {
	constructor() {}

	projectPath(projectId: string) {
		return `/${projectId}`;
	}
	settingsPath(projectId: string) {
		return `/${projectId}/settings`;
	}
	workspacePath(projectId: string) {
		return `/${projectId}/workspace`;
	}
	branchesPath(projectId: string) {
		return `/${projectId}/branches`;
	}
	targetPath(projectId: string) {
		return `/${projectId}/target`;
	}
	historyPath(projectId: string) {
		return `/${projectId}/history`;
	}
}
