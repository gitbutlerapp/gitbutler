/**
 * This file contains an injectable service for managing ui-specific feature flags.
 * The values are persisted in local storage. Entries are prefixed with 'feature'.
 *
 * @module uiFeatureFlags
 */
import { InjectionToken } from '@gitbutler/shared/context';
import { persisted, persistWithExpiration } from '@gitbutler/shared/persisted';
import { derived, type Readable } from 'svelte/store';

export const UI_FEATURE_FLAGS_SERVICE = new InjectionToken<UIFeatureFlagsService>('UIFeatureFlagsService');

export class UIFeatureFlagsService {
	readonly autoSelectBranchName = persisted(false, 'autoSelectBranchLaneContentsFeature');
	readonly ircEnabled = persistWithExpiration(false, 'feature-irc', 1440 * 30);
	readonly ircServer = persistWithExpiration('', 'feature-irc-server', 1440 * 30);
	readonly rewrapCommitMessage = persistWithExpiration(true, 'rewrap-commit-msg', 1440 * 30);
	readonly codegenEnabled = persistWithExpiration(false, 'feature-codegen', 1440 * 30);

	readonly flags: Readable<UIFeatureFlags> = derived(
		[this.autoSelectBranchName, this.ircEnabled, this.ircServer, this.rewrapCommitMessage, this.codegenEnabled],
		([autoSelectBranchName, ircEnabled, ircServer, rewrapCommitMessage, codegenEnabled]) => ({
			autoSelectBranchName,
			ircEnabled,
			ircServer,
			rewrapCommitMessage,
			codegenEnabled
		})
	);

	setAutoSelectBranchName(value: boolean) {
		this.autoSelectBranchName.set(value);
	}

	setIrcEnabled(value: boolean) {
		this.ircEnabled.set(value);
	}

	setIrcServer(value: string) {
		this.ircServer.set(value);
	}

	setRewrapCommitMessage(value: boolean) {
		this.rewrapCommitMessage.set(value);
	}

	setCodegenEnabled(value: boolean) {
		this.codegenEnabled.set(value);
	}
}

export interface UIFeatureFlags {
	autoSelectBranchName: boolean;
	ircEnabled: boolean;
	ircServer: string;
	rewrapCommitMessage: boolean;
	codegenEnabled: boolean;
}

// Deprecated exports - use UIFeatureFlagsService instead
export const autoSelectBranchNameFeature = persisted(false, 'autoSelectBranchLaneContentsFeature');
export const ircEnabled = persistWithExpiration(false, 'feature-irc', 1440 * 30);
export const ircServer = persistWithExpiration('', 'feature-irc-server', 1440 * 30);
export const rewrapCommitMessage = persistWithExpiration(true, 'rewrap-commit-msg', 1440 * 30);
export const codegenEnabled = persistWithExpiration(false, 'feature-codegen', 1440 * 30);
