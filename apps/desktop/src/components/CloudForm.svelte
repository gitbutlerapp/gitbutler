<script lang="ts">
	import AiPromptSelect from '$components/AIPromptSelect.svelte';
	import SettingsSection from '$components/SettingsSection.svelte';
	import WelcomeSigninAction from '$components/WelcomeSigninAction.svelte';
	import { projectAiExperimentalFeaturesEnabled, projectAiGenEnabled } from '$lib/config/config';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { Button, CardGroup, Spacer, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const userService = inject(USER_SERVICE);
	const user = userService.user;
	const { openGeneralSettings } = useSettingsModal();

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const experimentalAiGenEnabled = $derived(projectAiExperimentalFeaturesEnabled(projectId));
</script>

<SettingsSection>
	{#snippet description()}
		{$t('settings.project.ai.description')}
	{/snippet}

	<Spacer />

	{#if !$user}
		<WelcomeSigninAction />
		<Spacer />
	{/if}

	<CardGroup>
		<CardGroup.Item labelFor="aiGenEnabled">
			{#snippet title()}
				{$t('settings.project.ai.enableGeneration.title')}
			{/snippet}
			{#snippet caption()}
				{$t('settings.project.ai.enableGeneration.caption')}
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="aiGenEnabled"
					checked={$aiGenEnabled}
					onclick={() => {
						$aiGenEnabled = !$aiGenEnabled;
					}}
				/>
			{/snippet}
		</CardGroup.Item>
	</CardGroup>

	{#if $aiGenEnabled}
		<CardGroup>
			<CardGroup.Item labelFor="aiExperimental">
				{#snippet title()}
					{$t('settings.project.ai.enableExperimental.title')}
				{/snippet}
				{#snippet caption()}
					{$t('settings.project.ai.enableExperimental.caption')}
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="aiExperimental"
						checked={$experimentalAiGenEnabled}
						onclick={() => {
							$experimentalAiGenEnabled = !$experimentalAiGenEnabled;
						}}
					/>
				{/snippet}
			</CardGroup.Item>
		</CardGroup>
	{/if}

	<CardGroup>
		<CardGroup.Item>
			{#snippet title()}
				{$t('settings.project.ai.customPrompts.title')}
			{/snippet}

			<AiPromptSelect {projectId} promptUse="commits" />
			<AiPromptSelect {projectId} promptUse="branches" />

			<Spacer margin={8} />

			<p class="text-12 text-body">
				{$t('settings.project.ai.customPrompts.description')}
			</p>
			<Button kind="outline" icon="edit" onclick={() => openGeneralSettings('ai')}
				>{$t('settings.project.ai.customPrompts.button')}</Button
			>
		</CardGroup.Item>
	</CardGroup>
</SettingsSection>
