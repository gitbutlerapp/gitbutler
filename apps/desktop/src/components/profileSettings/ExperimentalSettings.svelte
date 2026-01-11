<script lang="ts">
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import {
		ircEnabled,
		ircServer,
		fModeEnabled,
		useNewRebaseEngine
	} from '$lib/config/uiFeatureFlags';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Textbox, Toggle } from '@gitbutler/ui';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

	const user = inject(USER);
</script>

<p class="text-12 text-body experimental-settings__text">
	{@html $t('settings.general.experimental.about')}
</p>

<CardGroup>
	<CardGroup.Item labelFor="apply3">
		{#snippet title()}
			{$t('settings.general.experimental.apply3.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.experimental.apply3.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="apply3"
				checked={$settingsStore?.featureFlags.apply3}
				onclick={() =>
					settingsService.updateFeatureFlags({ apply3: !$settingsStore?.featureFlags.apply3 })}
			/>
		{/snippet}
	</CardGroup.Item>
	<CardGroup.Item labelFor="f-mode">
		{#snippet title()}
			{$t('settings.general.experimental.fMode.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.experimental.fMode.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="f-mode"
				checked={$fModeEnabled}
				onclick={() => fModeEnabled.set(!$fModeEnabled)}
			/>
		{/snippet}
	</CardGroup.Item>
	<CardGroup.Item labelFor="new-rebase-engine">
		{#snippet title()}
			{$t('settings.general.experimental.newRebase.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.experimental.newRebase.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="new-rebase-engine"
				checked={$useNewRebaseEngine}
				onclick={() => useNewRebaseEngine.set(!$useNewRebaseEngine)}
			/>
		{/snippet}
	</CardGroup.Item>

	{#if $user?.role === 'admin'}
		<CardGroup.Item labelFor="single-branch">
			{#snippet title()}
				{$t('settings.general.experimental.singleBranch.title')}
			{/snippet}
			{#snippet caption()}
				{$t('settings.general.experimental.singleBranch.caption')}
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="single-branch"
					checked={$settingsStore?.featureFlags.singleBranch}
					onclick={() =>
						settingsService.updateFeatureFlags({
							singleBranch: !$settingsStore?.featureFlags.singleBranch
						})}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="irc">
			{#snippet title()}
				{$t('settings.general.experimental.irc.title')}
			{/snippet}
			{#snippet caption()}
				{$t('settings.general.experimental.irc.caption')}
			{/snippet}
			{#snippet actions()}
				<Toggle id="irc" checked={$ircEnabled} onclick={() => ($ircEnabled = !$ircEnabled)} />
			{/snippet}
		</CardGroup.Item>
		{#if $ircEnabled}
			<CardGroup.Item>
				<Textbox
					value={$ircServer}
					size="large"
					label={$t('settings.general.experimental.irc.serverLabel')}
					placeholder="wss://irc.gitbutler.com:443"
					onchange={(value) => ($ircServer = value)}
				/>
			</CardGroup.Item>
		{/if}
	{/if}
</CardGroup>

<style>
	.experimental-settings__text {
		margin-bottom: 10px;
		color: var(--clr-text-2);
	}
</style>
