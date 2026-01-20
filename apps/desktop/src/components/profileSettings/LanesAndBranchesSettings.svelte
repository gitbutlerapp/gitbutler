<script lang="ts">
	import {
		autoSelectBranchNameFeature,
		autoSelectBranchCreationFeature,
		stagingBehaviorFeature,
		type StagingBehavior
	} from '$lib/config/uiFeatureFlags';
	import { persisted } from '@gitbutler/shared/persisted';
	import { CardGroup, RadioButton, Toggle } from '@gitbutler/ui';

	const addToLeftmost = persisted<boolean>(false, 'branch-placement-leftmost');
	function onStagingBehaviorFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		const selectedStagingBehavior = formData.get('stagingBehaviorType') as StagingBehavior | null;
		if (!selectedStagingBehavior) return;
		stagingBehaviorFeature.set(selectedStagingBehavior);
	}</script>

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

<CardGroup>
	<form class="stack-v" onchange={(e) => onStagingBehaviorFormChange(e.currentTarget)}>
		<CardGroup.Item labelFor="stage-all">
			{#snippet title()}
				Stage all files
			{/snippet}
			{#snippet caption()}
				Stage all files assigned to the stack on commit. If no files are staged, all unassigned
				files will be staged.
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="stagingBehaviorType"
					value="all"
					id="stage-all"
					checked={$stagingBehaviorFeature === 'all'}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="stage-selection">
			{#snippet title()}
				Stage selected files
			{/snippet}
			{#snippet caption()}
				Stage the selected assigned files to the stack on commit. If no files are selected, stage
				all files. If there are no assigned files, stage all selected unassigned files.
				<br />
				And if no files are selected, stage all unassigned files.
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="stagingBehaviorType"
					value="selection"
					id="stage-selection"
					checked={$stagingBehaviorFeature === 'selection'}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="stage-none">
			{#snippet title()}
				Don't stage files automatically
			{/snippet}
			{#snippet caption()}
				Do not stage any files automatically.
				<br />
				You're more of a DIY developer in that way.
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="stagingBehaviorType"
					value="none"
					id="stage-none"
					checked={$stagingBehaviorFeature === 'none'}
				/>
			{/snippet}
		</CardGroup.Item>
	</form>
</CardGroup>
