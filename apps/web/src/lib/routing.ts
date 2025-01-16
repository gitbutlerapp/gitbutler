export interface OwnerParameters {
	ownerSlug: string;
}

export interface ProjectParameters extends OwnerParameters {
	projectSlug: string;
}

export interface ProjectReviewParameters extends ProjectParameters {
	branchId: string;
}

export interface ProjectReviewCommitParameters extends ProjectReviewParameters {
	changeId: string;
}

export function projectsPath() {
	return `/repositories`;
}

export function ownerPath(parameters: OwnerParameters) {
	return `/${parameters.ownerSlug}`;
}

export function projectPath(parameters: ProjectParameters) {
	return `/${parameters.ownerSlug}/${parameters.projectSlug}`;
}

export function projectReviewPath(parameters: ProjectParameters) {
	return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews`;
}

export function projectReviewBranchPath(parameters: ProjectReviewParameters) {
	return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}`;
}

export function projectReviewBranchCommitPath(parameters: ProjectReviewCommitParameters) {
	return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}/commit/${parameters.changeId}`;
}
