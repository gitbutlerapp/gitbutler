<script lang="ts">
	import { persistedDismissedForgeIntegrationPrompt } from '$lib/config/config';
	import {
		availableForgeDocsLink,
		availableForgeLabel,
		availableForgeReviewUnit,
		DEFAULT_FORGE_FACTORY,
		type AvailableForge
	} from '$lib/forge/forgeFactory.svelte';
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Button, Link } from '@gitbutler/ui';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const { openGeneralSettings, openProjectSettings } = useSettingsModal();
	const forgeFactory = inject(DEFAULT_FORGE_FACTORY);
	const dismissedTheIntegrationPrompt = $derived(
		persistedDismissedForgeIntegrationPrompt(projectId)
	);

	function configureIntegration(forge: AvailableForge): true {
		switch (forge) {
			case 'github':
				openGeneralSettings('integrations');
				return true;
			case 'gitlab':
				openProjectSettings(projectId);
				return true;
		}
	}

	function dismissPrompt() {
		dismissedTheIntegrationPrompt.set(true);
	}
</script>

{#if forgeFactory.canSetupIntegration && !$dismissedTheIntegrationPrompt}
	{@const forgeName = forgeFactory.canSetupIntegration}
	{@const forgeLabel = availableForgeLabel(forgeName)}
	{@const forgeUnit = availableForgeReviewUnit(forgeName)}
	{@const integrationDocs = availableForgeDocsLink(forgeName)}

	<div class="unassigned-view-prompt">
		<div class="unassigned-view-prompt__header">
			<h3 class="text-13">It looks like you have a {forgeLabel} remote!</h3>
		</div>
		<div class="unassigned-view-prompt__body">
			<p class="text-13">
				GitButler can display, create and manage {forgeUnit} for you directly in the app.
				<Link href={integrationDocs}>Read more</Link>
			</p>
		</div>

		<div class="unassigned-view-prompt__footer">
			<Button style="pop" onclick={() => configureIntegration(forgeName)}>Configure</Button>
			<Button kind="outline" onclick={dismissPrompt}>Dismiss</Button>
		</div>
	</div>
{/if}

<style lang="postcss">
	.unassigned-view-prompt {
		display: flex;
		flex-direction: column;
		width: auto;
		margin: 4px;
		padding: 8px;
		gap: 8px;
		border: 1px solid var(--clr-border-1);
		border-radius: var(--radius-m);
	}

	.unassigned-view-prompt__footer {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
