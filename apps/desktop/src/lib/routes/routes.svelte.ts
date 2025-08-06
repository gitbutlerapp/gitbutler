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

export function newProjectSettingsPath(projectId: string, page?: string) {
	if (page) {
		return `/${projectId}/new-settings/${page}`;
	}
	return `/${projectId}/new-settings`;
}

export function isNewProjectSettingsPath() {
	return isUrl<{ projectId: string }>('/[projectId]/new-settings/[[selectedId]]');
}

export function isProjectSettingsPath() {
	return isUrl<{ projectId: string }>('/[projectId]/settings');
}

export function workspacePath(projectId: string) {
	return `/${projectId}/workspace`;
}

export function ircPath(projectId: string) {
	return `/${projectId}/irc`;
}

export function isIrcPath() {
	return isUrl<{ projectId: string }>('/[projectId]/irc');
}

export function isWorkspacePath(): { projectId: string; stackId?: string } | undefined {
	const isStackUrl = isUrl<{ projectId: string; stackId?: string }>(
		'/[projectId]/workspace?stackId=[stackId]'
	);
	const isWorkspaceUrl = isUrl<{ projectId: string }>('/[projectId]/workspace');
	return isStackUrl ?? isWorkspaceUrl;
}

export function historyPath(projectId: string) {
	return `/${projectId}/history`;
}

export function isHistoryPath() {
	return isUrl<{ projectId: string }>('/[projectId]/history');
}

export function branchesPath(projectId: string) {
	return `/${projectId}/branches`;
}

export function isBranchesPath() {
	return isUrl<{ projectId: string }>('/[projectId]/branches');
}

export function vibeCenterPath(projectId: string) {
	return `/${projectId}/vibe-center`;
}

export function isVibeCenterPath() {
	return isUrl<{ projectId: string }>('/[projectId]/vibe-center');
}

export function isPreviewStackPath() {
	return isUrl<{ projectId: string }>('/[projectId]/preview-stack/[stackId]');
}

export function previewStackPath(projectId: string, stackId: string) {
	return `/${projectId}/preview-stack/${stackId}`;
}

export function isCommitPath() {
	return page.url.searchParams.has('create');
}

export function settingsPath() {
	return `/settings`;
}

export function newSettingsPath(page?: string) {
	if (page) {
		return `/new-settings/${page}`;
	}
	return `/new-settings`;
}

export function clonePath() {
	return '/onboarding/clone';
}
