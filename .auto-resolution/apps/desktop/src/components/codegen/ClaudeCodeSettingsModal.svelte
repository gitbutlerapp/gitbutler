<script lang="ts">
	import ClaudeCheck from '$components/v3/ClaudeCheck.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { inject } from '@gitbutler/core/context';
	import { Modal, SectionCard, Toggle } from '@gitbutler/ui';
	import type { Modal as ModalType } from '@gitbutler/ui';

	type Props = {
		onClose: () => void;
	};
	const { onClose }: Props = $props();

	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

	let claudeExecutable = $state('');
	let notifyOnCompletion = $state(false);
	let notifyOnPermissionRequest = $state(false);
	let dangerouslyAllowAllPermissions = $state(false);

	// Initialize Claude settings from store
	$effect(() => {
		if ($settingsStore?.claude) {
			claudeExecutable = $settingsStore.claude.executable;
			notifyOnCompletion = $settingsStore.claude.notifyOnCompletion;
			notifyOnPermissionRequest = $settingsStore.claude.notifyOnPermissionRequest;
			dangerouslyAllowAllPermissions = $settingsStore.claude.dangerouslyAllowAllPermissions;
		}
	});

	let recheckedAvailability = $state<'recheck-failed' | 'recheck-succeeded'>();
	async function checkClaudeAvailability() {
		const recheck = await claudeCodeService.fetchCheckAvailable(undefined, { forceRefetch: true });
		if (recheck) {
			recheckedAvailability = 'recheck-succeeded';
		} else {
			recheckedAvailability = 'recheck-failed';
		}
	}

	async function updateClaudeExecutable(value: string) {
		claudeExecutable = value;
		recheckedAvailability = undefined;
		await settingsService.updateClaude({ executable: value });
	}

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

	let modal: ModalType;

	export function show() {
		modal?.show();
	}

	export function close() {
		return modal?.close();
	}
</script>

<Modal bind:this={modal} width="medium" {onClose} title="Claude Code Settings" closeButton>
	{#snippet children(_item, _close)}
		<div class="settings-content">
			<SectionCard orientation="column">
				{#snippet title()}
					Claude Code Configuration
				{/snippet}

				{#snippet caption()}
					Configure the path to the Claude Code executable. This is used for AI-powered code
					generation and editing.
				{/snippet}

				<ClaudeCheck
					{claudeExecutable}
					{recheckedAvailability}
					onUpdateExecutable={updateClaudeExecutable}
					onCheckAvailability={checkClaudeAvailability}
					showInstallationGuide={false}
					showTitle={false}
				/>
			</SectionCard>

			<SectionCard orientation="row">
				{#snippet title()}
					Claude Code notifications
				{/snippet}
				{#snippet caption()}
					<div class="notification-toggles">
						<div class="notification-toggle">
							<p>Notify when Claude Code finishes</p>
							<Toggle checked={notifyOnCompletion} onchange={updateNotifyOnCompletion} />
						</div>
						<div class="notification-toggle">
							<p>Notify when Claude Code needs permission</p>
							<Toggle
								checked={notifyOnPermissionRequest}
								onchange={updateNotifyOnPermissionRequest}
							/>
						</div>
					</div>
				{/snippet}
				{#snippet actions()}{/snippet}
			</SectionCard>

			<SectionCard orientation="row">
				{#snippet title()}
					⚠️ Dangerously allow all permissions
				{/snippet}
				{#snippet caption()}
					Skips all permission prompts and allows Claude Code unrestricted access. Use with extreme
					caution.
				{/snippet}
				{#snippet actions()}
					<Toggle
						checked={dangerouslyAllowAllPermissions}
						onchange={updateDangerouslyAllowAllPermissions}
					/>
				{/snippet}
			</SectionCard>
		</div>
	{/snippet}
</Modal>

<style>
	.settings-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.notification-toggles {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.notification-toggle {
		display: flex;
		justify-content: space-between;
		gap: 8px;
	}
</style>
