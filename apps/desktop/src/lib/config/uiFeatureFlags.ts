/**
 * This file contains functions for managing ui-specific feature flags.
 * The values are persisted in local storage. Entries are prefixed with 'feature'.
 *
 * @module appSettings
 */
import { persisted, persistWithExpiration } from '@gitbutler/shared/persisted';

export const autoSelectBranchNameFeature = persisted(false, 'autoSelectBranchLaneContentsFeature');
export const workspaceSwapPanels = persisted<
	'dont-swap-panels' | 'swap-middle-to-right' | 'swap-middle-to-left'
>('dont-swap-panels', 'workspace-swap-panels');

export const ircEnabled = persistWithExpiration(false, 'feature-irc', 1440 * 30);
export const ircServer = persistWithExpiration('', 'feature-irc-server', 1440 * 30);
export const rewrapCommitMessage = persistWithExpiration(true, 'rewrap-commit-msg', 1440 * 30);
