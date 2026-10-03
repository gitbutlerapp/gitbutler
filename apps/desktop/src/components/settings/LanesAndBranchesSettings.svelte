<script lang="ts">
	import {
		autoSelectBranchNameFeature,
		autoSelectBranchCreationFeature,
		stagingBehaviorFeature,
		type StagingBehavior,
	} from "$lib/config/uiFeatureFlags";
	import { t } from "$lib/i18n";

	import { persisted } from "@gitbutler/shared/persisted";
	import { CardGroup, RadioButton, Toggle, Spacer } from "@gitbutler/ui-svelte";

	const addToLeftmost = persisted<boolean>(false, "branch-placement-leftmost");
	function onStagingBehaviorFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		const selectedStagingBehavior = formData.get("stagingBehaviorType") as StagingBehavior | null;
		if (!selectedStagingBehavior) return;
		stagingBehaviorFeature.set(selectedStagingBehavior);
	}
</script>

<CardGroup.Item standalone labelFor="add-leftmost">
	{#snippet title()}
		{t("place-new-lanes-on-the-left-side")}
	{/snippet}
	{#snippet caption()}
		{t("by-default-new-lanes-are-added-to-the-rightmost-position-ena")}
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
			{t("auto-select-text-on-branch-creation")}
		{/snippet}
		{#snippet caption()}
			{t("automatically-select-the-pre-populated-text-in-the-branch-na")}
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
			{t("auto-select-text-on-branch-rename")}
		{/snippet}
		{#snippet caption()}
			{t("automatically-select-the-text-when-renaming-a-branch-or-lane")}
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

<Spacer />

<div class="stack-v gap-8">
	<h2 class="text-15 text-bold">{t("commit-staging-behavior")}</h2>
	<p class="text-12 text-body clr-text-2">
		{t("controls-which-files-are-pre-selected-when-opening-the-stagi")}
		<br />
		{t("you-can-always-change-the-selection-manually")}
	</p>
</div>

<CardGroup>
	<form class="stack-v" onchange={(e) => onStagingBehaviorFormChange(e.currentTarget)}>
		<CardGroup.Item labelFor="stage-all">
			{#snippet title()}
				{t("auto-select-all-assigned-files")}
			{/snippet}
			{#snippet caption()}
				{t("pre-selects-all-files-assigned-to-this-branch-falls-back-to-")}
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="stagingBehaviorType"
					value="all"
					id="stage-all"
					checked={$stagingBehaviorFeature === "all"}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="stage-selection">
			{#snippet title()}
				{t("auto-select-only-your-picked-files")}
			{/snippet}
			{#snippet caption()}
				{t("pre-selects-only-the-files-you-have-already-picked-falls-bac")}
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="stagingBehaviorType"
					value="selection"
					id="stage-selection"
					checked={$stagingBehaviorFeature === "selection"}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="stage-none">
			{#snippet title()}
				{t("no-auto-selection")}
			{/snippet}
			{#snippet caption()}
				{t("nothing-is-pre-selected-you-manually-pick-what-to-include-in")}
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="stagingBehaviorType"
					value="none"
					id="stage-none"
					checked={$stagingBehaviorFeature === "none"}
				/>
			{/snippet}
		</CardGroup.Item>
	</form>
</CardGroup>
