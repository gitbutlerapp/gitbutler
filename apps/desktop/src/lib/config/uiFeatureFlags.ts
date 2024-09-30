/**
 * This file contains functions for managing ui-specific feature flags.
 * The values are persisted in local storage. Entries are prefixed with 'feature'.
 *
 * @module appSettings
 */
import { persisted, type Persisted } from '$lib/persisted/persisted';

export function featureBaseBranchSwitching(): Persisted<boolean> {
	const key = 'featureBaseBranchSwitching';
	return persisted(false, key);
}

export const stackingFeature = persisted(false, 'stackingFeature');

export function featureTopics(): Persisted<boolean> {
	const key = 'feature--topics';
	return persisted(false, key);
}

export const autoSelectBranchNameFeature = persisted(false, 'autoSelectBranchLaneContentsFeature');
