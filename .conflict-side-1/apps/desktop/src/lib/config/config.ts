import {
	persisted,
	getBooleanStorageItem,
	setStorageItem,
	persistWithExpiration,
	type Persisted
} from '@gitbutler/shared/persisted';

export function projectCommitGenerationExtraConcise(projectId: string): Persisted<boolean> {
	const key = 'projectCommitGenerationExtraConcise_';
	return persisted(false, key + projectId);
}

export function projectCommitGenerationHaiku(projectId: string): Persisted<boolean> {
	const key = 'projectCommitGenerationHaiku_';
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

export function projectAiGenEnabled(projectId: string): Persisted<boolean> {
	const key = 'projectAiGenEnabled_';
	return persisted(false, key + projectId);
}

export function projectAiExperimentalFeaturesEnabled(projectId: string): Persisted<boolean> {
	const key = 'projectAiExperimentalFeaturesEnabled_';
	return persisted(false, key + projectId);
}

export function projectRunCommitHooks(projectId: string): Persisted<boolean> {
	const key = 'projectRunCommitHooks_';
	return persisted(false, key + projectId);
}

export function persistedChatModelName<T extends string>(
	projectId: string,
	defaultValue: T
): Persisted<T> {
	const key = 'projectChatModelName_';
	return persisted(defaultValue, key + projectId);
}

const GITHUB_ORG_AUTH_ERROR_HANDLING_KEY = 'swallowGitHubOrgAuthErrors';
export function persistSwallowGitHubOrgAuthErrors(swallow: boolean) {
	setStorageItem(GITHUB_ORG_AUTH_ERROR_HANDLING_KEY, swallow);
}

export function getSwallowGitHubOrgAuthErrors(): boolean {
	return getBooleanStorageItem(GITHUB_ORG_AUTH_ERROR_HANDLING_KEY) ?? false;
}

export function persistedDismissedForgeIntegrationPrompt(projectId: string): Persisted<boolean> {
	const key = 'dismissedForgeIntegrationPrompt_';
	return persistWithExpiration(false, key + projectId, 48 * 60); // 48 hours
}
