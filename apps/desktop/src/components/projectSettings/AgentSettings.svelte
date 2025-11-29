<script lang="ts">
	import ClaudeCheck from '$components/codegen/ClaudeCheck.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { newlineOnEnter } from '$lib/config/uiFeatureFlags';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Link, Spacer, Toggle } from '@gitbutler/ui';

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
	Get the full guide to Agents in GitButler in <Link
		href="https://docs.gitbutler.com/features/agents-tab#installing-claude-code"
		>our documentation
	</Link>
</p>

<Spacer margin={10} dotted />

<CardGroup.Item standalone labelFor="autoCommitAfterCompletion">
	{#snippet title()}
		Auto-commit after completion
	{/snippet}
	{#snippet caption()}
		Automatically commit and rename branches when Claude Code finishes. Disable to review manually
		before committing.
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
		Use configured model
	{/snippet}
	{#snippet caption()}
		Use the model configured in .claude/settings.json.
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
		Newline on Enter
	{/snippet}
	{#snippet caption()}
		Use Enter for line breaks and Cmd+Enter to submit.
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
			Notify when finishes
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
			Notify when needs permission
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
		⚠ Dangerously allow all permissions
	{/snippet}
	{#snippet caption()}
		Skips all permission prompts and allows Claude Code unrestricted access. Use with extreme
		caution.
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="dangerouslyAllowAllPermissions"
			checked={dangerouslyAllowAllPermissions}
			onchange={updateDangerouslyAllowAllPermissions}
		/>
	{/snippet}
</CardGroup.Item>
