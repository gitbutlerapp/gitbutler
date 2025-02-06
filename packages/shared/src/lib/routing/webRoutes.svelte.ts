import { page } from '$app/state';

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
	messageUuid?: string;
}

function isUrl<T>(isWeb: boolean, id: string): T | undefined {
	if (!isWeb) return;

	if (page.route.id === id) {
		return page.params as T;
	}
}
function isUrlSubset<T>(isWeb: boolean, id: string): T | undefined {
	if (!isWeb) return;

	if (page.route.id?.startsWith(id)) {
		return page.params as T;
	}
}

export class WebRoutesService {
	constructor(
		private readonly baseUrl: string,
		private readonly _isWeb: boolean = false
	) {}

	private get isWeb() {
		return this._isWeb;
	}

	private toUrl(path: string) {
		return `${this.baseUrl}${path}`;
	}

	projectsPath() {
		return `/repositories`;
	}
	projectsUrl() {
		return this.toUrl(this.projectsPath());
	}
	// eslint-disable-next-line @typescript-eslint/no-empty-object-type
	isProjectsPage = $derived(isUrl<{}>(this.isWeb, '/repositories'));
	// eslint-disable-next-line @typescript-eslint/no-empty-object-type
	isProjectsPageSubset = $derived(isUrlSubset<{}>(this.isWeb, '/repositories'));

	ownerPath(parameters: OwnerParameters) {
		return `/${parameters.ownerSlug}`;
	}
	ownerUrl(parameters: OwnerParameters) {
		return this.toUrl(this.ownerPath(parameters));
	}
	isOwnerPage = $derived(isUrl<OwnerParameters>(this.isWeb, '/[ownerSlug]'));
	isOwnerPageSubset = $derived(isUrlSubset<OwnerParameters>(this.isWeb, '/[ownerSlug]'));

	projectPath(parameters: ProjectParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}`;
	}
	projectUrl(parameters: ProjectParameters) {
		return this.toUrl(this.projectPath(parameters));
	}
	isProjectPage = $derived(isUrl<ProjectParameters>(this.isWeb, '/[ownerSlug]/[projectSlug]'));
	isProjectPageSubset = $derived(
		isUrlSubset<ProjectParameters>(this.isWeb, '/[ownerSlug]/[projectSlug]')
	);

	projectReviewPath(parameters: ProjectParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews`;
	}
	projectReviewUrl(parameters: ProjectParameters) {
		return this.toUrl(this.projectReviewPath(parameters));
	}
	isProjectReviewPage = $derived(
		isUrl<ProjectParameters>(this.isWeb, '/[ownerSlug]/[projectSlug]/reviews')
	);
	isProjectReviewPageSubset = $derived(
		isUrlSubset<ProjectParameters>(this.isWeb, '/[ownerSlug]/[projectSlug]/reviews')
	);

	projectReviewBranchPath(parameters: ProjectReviewParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}`;
	}
	projectReviewBranchUrl(parameters: ProjectReviewParameters) {
		return this.toUrl(this.projectReviewBranchPath(parameters));
	}
	isProjectReviewBranchPage = $derived(
		isUrl<ProjectReviewParameters>(this.isWeb, '/[ownerSlug]/[projectSlug]/reviews/[branchId]')
	);
	isProjectReviewBranchPageSubset = $derived(
		isUrlSubset<ProjectReviewParameters>(
			this.isWeb,
			'/[ownerSlug]/[projectSlug]/reviews/[branchId]'
		)
	);

	projectReviewBranchCommitPath(parameters: ProjectReviewCommitParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}/commit/${parameters.changeId}`;
	}
	projectReviewBranchCommitUrl(parameters: ProjectReviewCommitParameters) {
		return this.toUrl(this.projectReviewBranchCommitPath(parameters));
	}
	isProjectReviewBranchCommitPage = $derived(
		isUrl<ProjectReviewCommitParameters>(
			this.isWeb,
			'/[ownerSlug]/[projectSlug]/reviews/[branchId]/commit/[changeId]'
		)
	);
	isProjectReviewBranchCommitPageSubset = $derived(
		isUrlSubset<ProjectReviewCommitParameters>(
			this.isWeb,
			'/[ownerSlug]/[projectSlug]/reviews/[branchId]/commit/[changeId]'
		)
	);
}
