/**
 * This file contains functions for managing ui-specific feature flags.
 * The values are persisted in local storage. Entries are prefixed with 'feature'.
 *
 * @module appSettings
 */
import { persisted } from '@gitbutler/shared/persisted';

export const stackingFeature = persisted(true, 'stackingFeature_v2');

export const stackingFeatureMultipleSeries = persisted(false, 'stackingFeatureMultipleSeries');

export const autoSelectBranchNameFeature = persisted(false, 'autoSelectBranchLaneContentsFeature');
