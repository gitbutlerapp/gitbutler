<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ClaudeCheck from '$components/v3/ClaudeCheck.svelte';
	import { useAvailabilityChecking } from '$lib/codegen/availabilityChecking.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { inject } from '@gitbutler/core/context';
	import { Modal, SectionCard, Toggle, Spacer } from '@gitbutler/ui';
	import type { Modal as ModalType } from '@gitbutler/ui';

	type Props = {
		onClose: () => void;
	};
	const { onClose }: Props = $props();

	const {
		claudeExecutable,
		recheckedAvailability,
		checkClaudeAvailability,
		updateClaudeExecutable
	} = useAvailabilityChecking();

	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

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

	let modal: ModalType;

	export function show() {
		modal?.show();
	}

	export function close() {
		return modal?.close();
	}
</script>

<Modal bind:this={modal} width={520} {onClose} noPadding>
	<ScrollableContainer>
		<div class="settings-content">
			<ClaudeCheck
				claudeExecutable={claudeExecutable.current}
				recheckedAvailability={recheckedAvailability.current}
				onUpdateExecutable={updateClaudeExecutable}
				onCheckAvailability={checkClaudeAvailability}
			/>

			<Spacer margin={10} />

			<SectionCard orientation="row" labelFor="autoCommitAfterCompletion">
				{#snippet title()}
					Auto-commit after completion
				{/snippet}
				{#snippet caption()}
					Automatically commit and rename branches when Claude Code finishes. Disable to review
					manually before committing.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="autoCommitAfterCompletion"
						checked={autoCommitAfterCompletion}
						onchange={updateAutoCommitAfterCompletion}
					/>
				{/snippet}
			</SectionCard>

			<SectionCard orientation="row" labelFor="useConfiguredModel">
				{#snippet title()}
					Use configured model
				{/snippet}
				{#snippet caption()}
					Use the model configured in .claude/settings.json.
					<br />
					Useful for 3rd party API providers.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="useConfiguredModel"
						checked={useConfiguredModel}
						onchange={updateUseConfiguredModel}
					/>
				{/snippet}
			</SectionCard>

			<SectionCard orientation="row" labelFor="dangerouslyAllowAllPermissions">
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
			</SectionCard>

			<Spacer margin={10} />

			<div class="stack-v">
				<SectionCard orientation="row" labelFor="notifyOnCompletion" roundedBottom={false}>
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
				</SectionCard>
				<SectionCard orientation="row" labelFor="notifyOnPermissionRequest" roundedTop={false}>
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
				</SectionCard>
			</div>
		</div>
	</ScrollableContainer>
</Modal>

<style lang="postcss">
	.settings-content {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 12px;
	}
</style>
