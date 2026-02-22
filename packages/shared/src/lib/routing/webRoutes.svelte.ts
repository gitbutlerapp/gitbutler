import { page } from '$app/state';
import { InjectionToken } from '@gitbutler/core/context';

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

export const WEB_ROUTES_SERVICE: InjectionToken<WebRoutesService> = new InjectionToken(
	'WebRoutesService'
);

export class WebRoutesService {
	constructor(
		private readonly baseUrl: string,
		private readonly _isWeb: boolean = false
	) {}

	private get isWeb() {
		return this._isWeb;
	}

	private toUrl(path: string) {
		const baseUrl = this.baseUrl.replace(/\/$/, '');
		if (baseUrl !== '' && (path === baseUrl || path.startsWith(`${baseUrl}/`))) {
			return path;
		}
		return `${baseUrl}${path}`;
	}

	private toBasePath(path: string) {
		const basePath = this.baseUrl === '/' ? '' : this.baseUrl.replace(/\/$/, '');
		return `${basePath}${path}`;
	}

	homePath() {
		return this.toBasePath('/');
	}
	homeUrl() {
		return this.toUrl(this.homePath());
	}

	loginPath() {
		return this.toBasePath('/login');
	}
	loginUrl() {
		return this.toUrl(this.loginPath());
	}

	resetPasswordPath() {
		return this.toBasePath('/login/forgot-password');
	}
	resetPasswordUrl() {
		return this.toUrl(this.resetPasswordPath());
	}

	signupPath() {
		return this.toBasePath('/signup');
	}
	signupUrl() {
		return this.toUrl(this.signupPath());
	}

	projectsPath() {
		return this.toBasePath('/');
	}
	projectsUrl() {
		return this.toUrl(this.projectsPath());
	}

	finalizeAccountPath() {
		return this.toBasePath('/profile/finalize');
	}
	finalizeAccountUrl() {
		return this.toUrl(this.finalizeAccountPath());
	}

	profilePath() {
		return this.toBasePath('/profile');
	}
	profileUrl() {
		return this.toUrl(this.profilePath());
	}

	// eslint-disable-next-line @typescript-eslint/no-empty-object-type
	isProjectsPage = $derived(isUrl<{}>(this.isWeb, '/projects'));
	// eslint-disable-next-line @typescript-eslint/no-empty-object-type
	isProjectsPageSubset = $derived(isUrlSubset<{}>(this.isWeb, '/projects'));

	ownerPath(parameters: OwnerParameters) {
		return this.toBasePath(`/${parameters.ownerSlug}`);
	}
	ownerUrl(parameters: OwnerParameters) {
		return this.toUrl(this.ownerPath(parameters));
	}
	isOwnerPage = $derived(isUrl<OwnerParameters>(this.isWeb, '/(app)/[ownerSlug]'));
	isOwnerPageSubset = $derived(isUrlSubset<OwnerParameters>(this.isWeb, '/(app)/[ownerSlug]'));

	projectPath(parameters: ProjectParameters) {
		return this.toBasePath(`/${parameters.ownerSlug}/${parameters.projectSlug}`);
	}
	projectUrl(parameters: ProjectParameters) {
		return this.toUrl(this.projectPath(parameters));
	}
	isProjectPage = $derived(
		isUrl<ProjectParameters>(this.isWeb, '/(app)/[ownerSlug]/[projectSlug]')
	);
	isProjectPageSubset = $derived(
		isUrlSubset<ProjectParameters>(this.isWeb, '/(app)/[ownerSlug]/[projectSlug]')
	);

	projectReviewPath(parameters: ProjectParameters) {
		return this.toBasePath(`/${parameters.ownerSlug}/${parameters.projectSlug}/reviews`);
	}
	projectReviewUrl(parameters: ProjectParameters) {
		return this.toUrl(this.projectReviewPath(parameters));
	}
	isProjectReviewPage = $derived(
		isUrl<ProjectParameters>(this.isWeb, '/(app)/[ownerSlug]/[projectSlug]/reviews')
	);
	isProjectReviewPageSubset = $derived(
		isUrlSubset<ProjectParameters>(this.isWeb, '/(app)/[ownerSlug]/[projectSlug]/reviews')
	);

	projectReviewBranchPath(parameters: ProjectReviewParameters) {
		return this.toBasePath(
			`/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}`
		);
	}
	projectReviewBranchUrl(parameters: ProjectReviewParameters) {
		return this.toUrl(this.projectReviewBranchPath(parameters));
	}
	isProjectReviewBranchPage = $derived(
		isUrl<ProjectReviewParameters>(
			this.isWeb,
			'/(app)/[ownerSlug]/[projectSlug]/reviews/[branchId]'
		)
	);
	isProjectReviewBranchPageSubset = $derived(
		isUrlSubset<ProjectReviewParameters>(
			this.isWeb,
			'/(app)/[ownerSlug]/[projectSlug]/reviews/[branchId]'
		)
	);

	projectReviewBranchCommitPath(parameters: ProjectReviewCommitParameters) {
		return this.toBasePath(
			`/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}/commit/${parameters.changeId}`
		);
	}
	projectReviewBranchCommitUrl(parameters: ProjectReviewCommitParameters) {
		return this.toUrl(this.projectReviewBranchCommitPath(parameters));
	}
	isProjectReviewBranchCommitPage = $derived(
		isUrl<ProjectReviewCommitParameters>(
			this.isWeb,
			'/(app)/[ownerSlug]/[projectSlug]/reviews/[branchId]/commit/[changeId]'
		)
	);
	isProjectReviewBranchCommitPageSubset = $derived(
		isUrlSubset<ProjectReviewCommitParameters>(
			this.isWeb,
			'/(app)/[ownerSlug]/[projectSlug]/reviews/[branchId]/commit/[changeId]'
		)
	);
}
