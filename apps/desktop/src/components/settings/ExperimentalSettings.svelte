<script lang="ts">
	import { fModeEnabled } from "$lib/config/uiFeatureFlags";
	import { t } from "$lib/i18n";

	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { USER_SERVICE } from "$lib/user/userService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Toggle } from "@gitbutler/ui-svelte";

	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;

	const userService = inject(USER_SERVICE);
</script>

<p class="text-12 text-body experimental-settings__text">
	{t("flags-for-features-in-development-or-beta-features-may-not-w")}
	<br />
	{t("use-at-your-own-risk")}
</p>

<CardGroup>
	<CardGroup.Item labelFor="f-mode">
		{#snippet title()}
			{t("f-mode-navigation")}
		{/snippet}
		{#snippet caption()}
			{t("enable-f-mode-for-quick-keyboard-navigation-to-buttons-using")}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="f-mode"
				checked={$fModeEnabled}
				onclick={() => fModeEnabled.set(!$fModeEnabled)}
			/>
		{/snippet}
	</CardGroup.Item>

	{#if userService.user?.role === "admin"}
		<CardGroup.Item labelFor="single-branch">
			{#snippet title()}
				{t("single-branch-mode")}
			{/snippet}
			{#snippet caption()}
				{t("stay-in-the-workspace-view-when-leaving-the-gitbutler-worksp")}
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="single-branch"
					checked={$settingsStore?.featureFlags.singleBranch}
					onclick={() =>
						settingsService.updateFeatureFlags({
							singleBranch: !$settingsStore?.featureFlags.singleBranch,
						})}
				/>
			{/snippet}
		</CardGroup.Item>
	{/if}

	<CardGroup.Item labelFor="worktree-manipulation">
		{#snippet title()}
			{t("worktree-manipulation")}
		{/snippet}
		{#snippet caption()}
			{t("enable-experimental-support-for-working-with-linked-git-work")}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="worktree-manipulation"
				checked={$settingsStore?.featureFlags.worktreeManipulation}
				onclick={() =>
					settingsService.updateFeatureFlags({
						worktreeManipulation: !$settingsStore?.featureFlags.worktreeManipulation,
					})}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<style>
	.experimental-settings__text {
		margin-bottom: 10px;
		color: var(--text-2);
	}
</style>
