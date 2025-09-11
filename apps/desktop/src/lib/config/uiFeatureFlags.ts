/**
 * This file contains functions for managing ui-specific feature flags.
 * The values are persisted in local storage. Entries are prefixed with 'feature'.
 *
 * @module appSettings
 */
import { persisted, persistWithExpiration } from '@gitbutler/shared/persisted';

export const autoSelectBranchNameFeature = persisted(false, 'autoSelectBranchLaneContentsFeature');
export const ircEnabled = persistWithExpiration(false, 'feature-irc', 1440 * 30);
export const ircServer = persistWithExpiration('', 'feature-irc-server', 1440 * 30);
export const rewrapCommitMessage = persistWithExpiration(true, 'rewrap-commit-msg', 1440 * 30);
// V2 to make sure it's going to set everyone's to true untill the turn it to false again.
export const codegenEnabled = persisted(true, 'feature-codegen-v2');
export type StagingBehavior = 'all' | 'selection' | 'none';
export const stagingBehaviorFeature = persisted<StagingBehavior>('all', 'feature-staging-behavior');
