<script lang="ts">
	import {
		autoSelectBranchNameFeature,
		autoSelectBranchCreationFeature
	} from '$lib/config/uiFeatureFlags';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { CardGroup, Toggle } from '@gitbutler/ui';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;

	const addToLeftmost = persisted<boolean>(false, 'branch-placement-leftmost');
</script>

<CardGroup.Item standalone labelFor="add-leftmost">
	{#snippet title()}
		{$t('settings.general.lanesAndBranches.newLanesPlacement.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.general.lanesAndBranches.newLanesPlacement.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="add-leftmost"
			checked={$addToLeftmost}
			onclick={() => ($addToLeftmost = !$addToLeftmost)}
		/>
	{/snippet}
</CardGroup.Item>

<CardGroup>
	<CardGroup.Item labelFor="auto-select-creation">
		{#snippet title()}
			{$t('settings.general.lanesAndBranches.autoSelectCreation.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.lanesAndBranches.autoSelectCreation.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="auto-select-creation"
				checked={$autoSelectBranchCreationFeature}
				onclick={() => ($autoSelectBranchCreationFeature = !$autoSelectBranchCreationFeature)}
			/>
		{/snippet}
	</CardGroup.Item>
	<CardGroup.Item labelFor="auto-select-rename">
		{#snippet title()}
			{$t('settings.general.lanesAndBranches.autoSelectRename.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.lanesAndBranches.autoSelectRename.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="auto-select-rename"
				checked={$autoSelectBranchNameFeature}
				onclick={() => ($autoSelectBranchNameFeature = !$autoSelectBranchNameFeature)}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>
