import { persisted, type Persisted } from '@gitbutler/shared/persisted';

export function projectHttpsWarningBannerDismissed(projectId: string): Persisted<boolean> {
	const key = 'projectHttpsWarningBannerDismissed_';
	return persisted(false, key + projectId);
}

export function projectCommitGenerationExtraConcise(projectId: string): Persisted<boolean> {
	const key = 'projectCommitGenerationExtraConcise_';
	return persisted(false, key + projectId);
}

export function projectCommitGenerationUseEmojis(projectId: string): Persisted<boolean> {
	const key = 'projectCommitGenerationUseEmojis_';
	return persisted(false, key + projectId);
}

export enum ListPRsFilter {
	All = 'ALL',
	ExcludeBots = 'EXCLUDE_BOTS',
	OnlyYours = 'ONLY_YOURS'
}

export function projectPullRequestListingFilter(projectId: string): Persisted<string> {
	const key = 'projectPullRequestListingFilter_';
	return persisted(ListPRsFilter.All, key + projectId);
}

export function projectAiGenEnabled(projectId: string): Persisted<boolean> {
	const key = 'projectAiGenEnabled_';
	return persisted(false, key + projectId);
}

export function projectRunCommitHooks(projectId: string): Persisted<boolean> {
	const key = 'projectRunCommitHooks_';
	return persisted(false, key + projectId);
}

export function projectLaneCollapsed(projectId: string, laneId: string): Persisted<boolean> {
	const key = 'projectLaneCollapsed_';
	return persisted(false, key + projectId + '_' + laneId);
}

export function persistedCommitMessage(projectId: string, branchId: string): Persisted<string> {
	return persisted('', 'projectCurrentCommitMessage_' + projectId + '_' + branchId);
}

export const showStackingCardDetails = persisted(false, 'showStackingCardDetails');

export const showHistoryView = persisted(false, 'showHistoryView');
