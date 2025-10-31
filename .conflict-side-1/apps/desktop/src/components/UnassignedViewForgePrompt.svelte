<script lang="ts">
	import githubLogoSvg from '$lib/assets/unsized-logos/github.svg?raw';
	import gitlabLogoSvg from '$lib/assets/unsized-logos/gitlab.svg?raw';
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

	<!-- <div class="forge-prompt__wrap" class:border-bottom={bottomBorder} class:border-top={topBorder}> -->
	<div class="forge-prompt">
		<div class="forge-prompt__logo">
			{@html forgeName === 'github' ? githubLogoSvg : gitlabLogoSvg}
		</div>
		<h3 class="text-13 text-body text-bold">It looks like you have a {forgeLabel} remote!</h3>
		<p class="text-12 text-body m-b-8 clr-text-2">
			GitButler can display, create and manage {forgeUnit} for you directly in the app.
			<Link href={integrationDocs}>Read more</Link>
		</p>

		<div class="forge-prompt__footer">
			<Button kind="outline" onclick={dismissPrompt}>Dismiss</Button>
			<Button style="pop" onclick={() => configureIntegration(forgeName)}
				>Configure integration…</Button
			>
		</div>
	</div>
	<!-- </div> -->
{/if}

<style lang="postcss">
	.forge-prompt {
		display: flex;
		z-index: 1;
		flex-direction: column;
		margin-bottom: -1px;
		padding: 14px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	.forge-prompt__logo {
		width: 22px;
		height: 22px;
		fill: var(--clr-text-2);
	}

	.forge-prompt__footer {
		display: flex;
		justify-content: flex-end;
		gap: 6px;
	}
</style>
