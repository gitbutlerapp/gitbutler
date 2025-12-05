/**
 * This file contains functions for managing ui-specific feature flags.
 * The values are persisted in local storage. Entries are prefixed with 'feature'.
 *
 * @module appSettings
 */
import { persisted, persistWithExpiration } from '@gitbutler/shared/persisted';

export const autoSelectBranchNameFeature = persisted(false, 'autoSelectBranchLaneContentsFeature');
export const autoSelectBranchCreationFeature = persisted(false, 'autoSelectBranchCreationFeature');
export const ircEnabled = persistWithExpiration(false, 'feature-irc', 1440 * 30);
export const ircServer = persistWithExpiration('', 'feature-irc-server', 1440 * 30);
export const rewrapCommitMessage = persistWithExpiration(true, 'rewrap-commit-msg', 1440 * 30);
export type StagingBehavior = 'all' | 'selection' | 'none';
export const stagingBehaviorFeature = persisted<StagingBehavior>('all', 'feature-staging-behavior');
export const fModeEnabled = persisted(true, 'f-mode');
export const newlineOnEnter = persisted(false, 'feature-newline-on-enter');
export const useNewRebaseEngine = persisted(false, 'feature-use-new-rebase-engine');
