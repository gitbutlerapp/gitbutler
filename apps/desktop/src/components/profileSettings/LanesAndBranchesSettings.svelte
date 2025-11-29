<script lang="ts">
	import {
		autoSelectBranchNameFeature,
		autoSelectBranchCreationFeature
	} from '$lib/config/uiFeatureFlags';
	import { persisted } from '@gitbutler/shared/persisted';
	import { CardGroup, Toggle } from '@gitbutler/ui';

	const addToLeftmost = persisted<boolean>(false, 'branch-placement-leftmost');
</script>

<CardGroup.Item standalone labelFor="add-leftmost">
	{#snippet title()}
		Place new lanes on the left side
	{/snippet}
	{#snippet caption()}
		By default, new lanes are added to the rightmost position. Enable this to add them to the
		leftmost position instead.
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
			Auto-select text on branch creation
		{/snippet}
		{#snippet caption()}
			Automatically select the pre-populated text in the branch name field when creating a new
			branch, making it easier to type your own name.
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
			Auto-select text on branch rename
		{/snippet}
		{#snippet caption()}
			Automatically select the text when renaming a branch or lane, making it easier to replace the
			entire name.
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
