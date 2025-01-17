/**
 * This file contains functions for managing ui-specific feature flags.
 * The values are persisted in local storage. Entries are prefixed with 'feature'.
 *
 * @module appSettings
 */
import { persisted } from '@gitbutler/shared/persisted';

export const v3 = persisted(false, 'featureFlag--v3');

export const autoSelectBranchNameFeature = persisted(false, 'autoSelectBranchLaneContentsFeature');

export const cloudFunctionality = persisted(false, 'featureFlag--cloudFunctionality');
export const cloudReviewFunctionality = persisted(false, 'featureFlag--cloudReviewFunctionality');
export const cloudCommunicationFunctionality = persisted(
	false,
	'featureFlag--cloudCommunicationFunctionality'
);
