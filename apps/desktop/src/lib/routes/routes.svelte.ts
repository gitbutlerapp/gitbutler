import { page } from '$app/state';

function isUrl<T>(id: string): T | undefined {
	if (page.route.id === id) {
		return page.params as T;
	}
}
export class DesktopRoutesService {
	constructor() {}

	projectPath(projectId: string) {
		return `/${projectId}`;
	}
	isProjectPath = $derived(isUrl<{ projectId: string }>('/[projectId]'));

	projectSettingsPath(projectId: string) {
		return `/${projectId}/settings`;
	}
	isProjectSettingsPath = $derived(isUrl<{ projectId: string }>('/[projectId]/settings'));

	workspacePath(projectId: string) {
		return `/${projectId}/workspace`;
	}
	isWorkspacePath = $derived(
		isUrl<{ projectId: string; branchId?: string }>('/[projectId]/workspace/[[stackId]]')
	);

	branchesPath(projectId: string) {
		return `/${projectId}/branches`;
	}
	isBranchesPath = $derived(isUrl<{ projectId: string }>('/[projectId]/branches'));

	targetPath(projectId: string) {
		return `/${projectId}/target`;
	}
	isTargetPath = $derived(isUrl<{ projectId: string }>('/[projectId]/target'));

	historyPath(projectId: string) {
		return `/${projectId}/history`;
	}
	isHistoryPath = $derived(isUrl<{ projectId: string }>('/[projectId]/history'));

	isCommitPath = $derived(
		isUrl<{ projectId: string; stackId: string }>('/[projectId]/workspace/[[stackId]]/commit')
	);
}

export function settingsPath() {
	return `/settings`;
}

export function stackPath(projectId: string, stackId: string) {
	return `/${projectId}/workspace/${stackId}`;
}

export function createCommitPath(projectId: string, stackId: string, branchName: string) {
	return `/${projectId}/workspace/${stackId}/${branchName}/commit`;
}

export function clonePath() {
	return '/onboarding/clone';
}

export function branchPath(projectId: string, stackId: string, branchName: string) {
	console.log(stackId, branchName);
	return `/${projectId}/workspace/${stackId}/${branchName}`;
}

export function commitPath(
	projectId: string,
	commitKey: { stackId: string; branchName: string; commitId: string; upstream: boolean }
) {
	const { stackId, branchName, commitId, upstream } = commitKey;
	const url = `/${projectId}/workspace/${stackId}/${branchName}?commitId=${commitId}`;
	return upstream ? url + '&upstream' : url;
}
