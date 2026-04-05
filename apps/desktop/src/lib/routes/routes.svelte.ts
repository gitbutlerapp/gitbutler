import { page } from "$app/state";

function isUrl<T>(id: string): T | undefined {
	if (page.route.id === id) {
		return page.params as T;
	}
}

function prefix(projectId: string): string {
	return page.data.projectPinned ? "" : `/${projectId}`;
}

export function projectPath(projectId: string) {
	return prefix(projectId) || "/";
}

export function isProjectPath() {
	return isUrl<{ projectId: string }>("/[[projectId=uuid]]");
}

export function workspacePath(projectId: string) {
	return `${prefix(projectId)}/workspace`;
}

export function isWorkspacePath(): { projectId: string; stackId?: string } | undefined {
	const isStackUrl = isUrl<{ projectId: string; stackId?: string }>(
		"/[[projectId=uuid]]/workspace?stackId=[stackId]",
	);
	const isWorkspaceUrl = isUrl<{ projectId: string }>("/[[projectId=uuid]]/workspace");
	return isStackUrl ?? isWorkspaceUrl;
}

export function historyPath(projectId: string) {
	return `${prefix(projectId)}/history`;
}

export function isHistoryPath() {
	return isUrl<{ projectId: string }>("/[[projectId=uuid]]/history");
}

export function branchesPath(projectId: string) {
	return `${prefix(projectId)}/branches`;
}

export function isBranchesPath() {
	return isUrl<{ projectId: string }>("/[[projectId=uuid]]/branches");
}

export function codegenPath(projectId: string) {
	return `${prefix(projectId)}/codegen`;
}

export function isCodegenPath() {
	return isUrl<{ projectId: string }>("/[[projectId=uuid]]/codegen");
}

export function isPreviewStackPath() {
	return isUrl<{ projectId: string }>("/[[projectId=uuid]]/preview-stack/[stackId]");
}

export function previewStackPath(projectId: string, stackId: string) {
	return `${prefix(projectId)}/preview-stack/${stackId}`;
}

export function isCommitPath() {
	return page.url.searchParams.has("create");
}

export function editModePath(projectId: string) {
	return `${prefix(projectId)}/edit`;
}

export function clonePath() {
	return "/onboarding/clone";
}
