<script lang="ts">
	import ClaudeCheck from '$components/codegen/ClaudeCheck.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { newlineOnEnter } from '$lib/config/uiFeatureFlags';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Spacer, Toggle } from '@gitbutler/ui';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

	// Agent settings state
	let notifyOnCompletion = $state(false);
	let notifyOnPermissionRequest = $state(false);
	let dangerouslyAllowAllPermissions = $state(false);
	let autoCommitAfterCompletion = $state(true);
	let useConfiguredModel = $state(false);

	// Initialize Claude settings from store
	$effect(() => {
		if ($settingsStore?.claude) {
			notifyOnCompletion = $settingsStore.claude.notifyOnCompletion;
			notifyOnPermissionRequest = $settingsStore.claude.notifyOnPermissionRequest;
			dangerouslyAllowAllPermissions = $settingsStore.claude.dangerouslyAllowAllPermissions;
			autoCommitAfterCompletion = $settingsStore.claude.autoCommitAfterCompletion;
			useConfiguredModel = $settingsStore.claude.useConfiguredModel;
		}
	});

	async function updateNotifyOnCompletion(value: boolean) {
		notifyOnCompletion = value;
		await settingsService.updateClaude({ notifyOnCompletion: value });
	}

	async function updateNotifyOnPermissionRequest(value: boolean) {
		notifyOnPermissionRequest = value;
		await settingsService.updateClaude({ notifyOnPermissionRequest: value });
	}

	async function updateDangerouslyAllowAllPermissions(value: boolean) {
		dangerouslyAllowAllPermissions = value;
		await settingsService.updateClaude({ dangerouslyAllowAllPermissions: value });
	}

	async function updateAutoCommitAfterCompletion(value: boolean) {
		autoCommitAfterCompletion = value;
		await settingsService.updateClaude({ autoCommitAfterCompletion: value });
	}

	async function updateUseConfiguredModel(value: boolean) {
		useConfiguredModel = value;
		await settingsService.updateClaude({ useConfiguredModel: value });
	}
</script>

<CardGroup.Item standalone>
	<ClaudeCheck showTitle />
</CardGroup.Item>

<p class="text-13 text-body clr-text-2">
	{@html $t('settings.project.agent.guideText')}
</p>

<Spacer margin={10} dotted />

<CardGroup.Item standalone labelFor="autoCommitAfterCompletion">
	{#snippet title()}
		{$t('settings.project.agent.autoCommit.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.project.agent.autoCommit.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="autoCommitAfterCompletion"
			checked={autoCommitAfterCompletion}
			onchange={updateAutoCommitAfterCompletion}
		/>
	{/snippet}
</CardGroup.Item>

<CardGroup.Item standalone labelFor="useConfiguredModel">
	{#snippet title()}
		{$t('settings.project.agent.useConfiguredModel.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.project.agent.useConfiguredModel.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="useConfiguredModel"
			checked={useConfiguredModel}
			onchange={updateUseConfiguredModel}
		/>
	{/snippet}
</CardGroup.Item>

<CardGroup.Item standalone labelFor="newlineOnEnter">
	{#snippet title()}
		{$t('settings.project.agent.newlineOnEnter.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.project.agent.newlineOnEnter.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="newlineOnEnter"
			checked={$newlineOnEnter}
			onchange={() => newlineOnEnter.set(!$newlineOnEnter)}
		/>
	{/snippet}
</CardGroup.Item>

<CardGroup>
	<CardGroup.Item labelFor="notifyOnCompletion">
		{#snippet title()}
			{$t('settings.project.agent.notifyOnCompletion')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="notifyOnCompletion"
				checked={notifyOnCompletion}
				onchange={updateNotifyOnCompletion}
			/>
		{/snippet}
	</CardGroup.Item>
	<CardGroup.Item labelFor="notifyOnPermissionRequest">
		{#snippet title()}
			{$t('settings.project.agent.notifyOnPermissionRequest')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="notifyOnPermissionRequest"
				checked={notifyOnPermissionRequest}
				onchange={updateNotifyOnPermissionRequest}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<Spacer margin={10} dotted />

<CardGroup.Item standalone labelFor="dangerouslyAllowAllPermissions">
	{#snippet title()}
		{$t('settings.project.agent.dangerousPermissions.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.project.agent.dangerousPermissions.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="dangerouslyAllowAllPermissions"
			checked={dangerouslyAllowAllPermissions}
			onchange={updateDangerouslyAllowAllPermissions}
		/>
	{/snippet}
</CardGroup.Item>
