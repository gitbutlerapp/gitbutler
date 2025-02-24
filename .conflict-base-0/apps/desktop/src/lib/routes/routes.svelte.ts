import { page } from '$app/state';

function isUrl<T>(id: string): T | undefined {
	if (page.route.id === id) {
		return page.params as T;
	}
}

export function projectPath(projectId: string) {
	return `/${projectId}`;
}

export function isProjectPath() {
	return isUrl<{ projectId: string }>('/[projectId]');
}

export function projectSettingsPath(projectId: string) {
	return `/${projectId}/settings`;
}

export function isProjectSettingsPath() {
	return isUrl<{ projectId: string }>('/[projectId]/settings');
}

export function workspacePath(projectId: string) {
	return `/${projectId}/workspace`;
}

export function isWorkspacePath() {
	return isUrl<{ projectId: string; branchId?: string }>('/[projectId]/workspace/[[stackId]]');
}

export function branchesPath(projectId: string) {
	return `/${projectId}/branches`;
}

export function isBranchesPath() {
	return isUrl<{ projectId: string }>('/[projectId]/branches');
}

export function targetPath(projectId: string) {
	return `/${projectId}/target`;
}

export function isTargetPath() {
	return isUrl<{ projectId: string }>('/[projectId]/target');
}

export function historyPath(projectId: string) {
	return `/${projectId}/history`;
}

export function isHistoryPath() {
	return isUrl<{ projectId: string }>('/[projectId]/history');
}

export function isCommitPath() {
	return isUrl<{ projectId: string; stackId: string }>('/[projectId]/workspace/[[stackId]]/commit');
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
