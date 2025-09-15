import { persisted, persistWithExpiration, type Persisted } from '@gitbutler/shared/persisted';

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

export function persistedDismissedForgeIntegrationPrompt(projectId: string): Persisted<boolean> {
	const key = 'dismissedForgeIntegrationPrompt_';
	return persistWithExpiration(false, key + projectId, 48 * 60); // 48 hours
}
