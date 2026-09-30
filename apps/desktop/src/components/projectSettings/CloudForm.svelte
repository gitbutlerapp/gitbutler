<script lang="ts">
	import AiPromptSelect from "$components/projectSettings/AIPromptSelect.svelte";
	import AccessTokenSignIn from "$components/shared/AccessTokenSignIn.svelte";
	import SettingsSection from "$components/shared/SettingsSection.svelte";
	import { projectAiExperimentalFeaturesEnabled, projectAiGenEnabled } from "$lib/config/config";
	import { t } from "$lib/i18n";
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { USER_SERVICE } from "$lib/user/userService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup, Spacer, Toggle } from "@gitbutler/ui-svelte";

	const { projectId }: { projectId: string } = $props();

	const userService = inject(USER_SERVICE);
	const { openGeneralSettings } = useSettingsModal();

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const experimentalAiGenEnabled = $derived(projectAiExperimentalFeaturesEnabled(projectId));
</script>

<SettingsSection>
	{#snippet description()}
		{t("gitbutler-supports-the-use-of-openai-and-anthropic-to-provid")}
	{/snippet}

	<Spacer />

	{#if !userService.user}
		<AccessTokenSignIn />
		<Spacer />
	{/if}

	<CardGroup>
		<CardGroup.Item labelFor="aiGenEnabled">
			{#snippet title()}
				{t("enable-branch-and-commit-message-generation")}
			{/snippet}
			{#snippet caption()}
				{t("if-enabled-diffs-will-be-sent-to-openai-or-anthropic-s-serve")}
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
					{t("enable-experimental-ai-features")}
				{/snippet}
				{#snippet caption()}
					{t("if-enabled-you-will-be-able-to-access-the-ai-features-curren")}
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
				{t("custom-prompts")}
			{/snippet}

			<AiPromptSelect {projectId} promptUse="commits" />
			<AiPromptSelect {projectId} promptUse="branches" />

			<Spacer margin={8} />

			<p class="text-12 text-body">
				{t("you-can-apply-your-own-custom-prompts-to-the-project-by-defa")}
			</p>
			<Button kind="outline" icon="edit" onclick={() => openGeneralSettings("ai")}
				>{t("customize-prompts")}</Button
			>
		</CardGroup.Item>
	</CardGroup>
</SettingsSection>
