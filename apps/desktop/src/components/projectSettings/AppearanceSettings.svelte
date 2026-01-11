<script lang="ts">
	import ThemeSelector from '$components/ThemeSelector.svelte';
	import { stagingBehaviorFeature, type StagingBehavior } from '$lib/config/uiFeatureFlags';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import {
		CardGroup,
		HunkDiff,
		RadioButton,
		Select,
		SelectItem,
		Textbox,
		Toggle
	} from '@gitbutler/ui';
	import type { ScrollbarVisilitySettings } from '@gitbutler/ui/components/scroll/Scrollbar.svelte';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;

	const userSettings = inject(SETTINGS);
	const diff = `@@ -56,10 +56,10 @@
			// Diff example
			projectName={project.title}
			{remoteBranches}
			on:branchSelected={async (e) => {
-				selectedBranch = e.detail;
-				if ($platformName === 'win32') {
+				if ($platformName === 'win64' && $userSettings.enableAdvancedFeatures) {
+					// Enhanced platform detection with feature flags
					setTarget();
				}
			}}`;

	function onScrollbarFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		const selectedScrollbarVisibility = formData.get(
			'scrollBarVisibilityType'
		) as ScrollbarVisilitySettings;

		userSettings.update((s) => ({
			...s,
			scrollbarVisibilityState: selectedScrollbarVisibility
		}));
	}

	function onStagingBehaviorFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		const selectedStagingBehavior = formData.get('stagingBehaviorType') as StagingBehavior | null;
		if (!selectedStagingBehavior) return;
		stagingBehaviorFeature.set(selectedStagingBehavior);
	}

	const fileListModeOptions = $derived([
		{ label: $t('settings.general.appearance.fileListMode.listView'), value: 'list' },
		{ label: $t('settings.general.appearance.fileListMode.treeView'), value: 'tree' }
	]);

	const linesContrastOptions = $derived([
		{ label: $t('settings.general.appearance.linesContrast.light'), value: 'light' },
		{ label: $t('settings.general.appearance.linesContrast.medium'), value: 'medium' },
		{ label: $t('settings.general.appearance.linesContrast.strong'), value: 'strong' }
	]);
</script>

<CardGroup.Item standalone>
	{#snippet title()}
		{$t('settings.general.appearance.theme.title')}
	{/snippet}
	<ThemeSelector {userSettings} />
</CardGroup.Item>

<CardGroup.Item alignment="center" standalone>
	{#snippet title()}
		{$t('settings.general.appearance.fileListMode.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.general.appearance.fileListMode.caption')}
	{/snippet}
	{#snippet actions()}
		<Select
			maxWidth={120}
			value={$userSettings.defaultFileListMode}
			options={fileListModeOptions}
			onselect={(value) => {
				userSettings.update((s) => ({
					...s,
					defaultFileListMode: value as 'tree' | 'list'
				}));
			}}
		>
			{#snippet itemSnippet({ item, highlighted })}
				<SelectItem selected={item.value === $userSettings.defaultFileListMode} {highlighted}>
					{item.label}
				</SelectItem>
			{/snippet}
		</Select>
	{/snippet}
</CardGroup.Item>

<CardGroup.Item labelFor="pathFirst" standalone>
	{#snippet title()}
		{$t('settings.general.appearance.filePathFirst.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.general.appearance.filePathFirst.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="pathFirst"
			checked={$userSettings.pathFirst}
			onclick={() => {
				userSettings.update((s) => ({
					...s,
					pathFirst: !s.pathFirst
				}));
			}}
		/>
	{/snippet}
</CardGroup.Item>

<CardGroup>
	<CardGroup.Item alignment="center">
		{#snippet title()}
			{$t('settings.general.appearance.diffPreview.title')}
		{/snippet}

		<HunkDiff
			filePath="test.tsx"
			tabSize={$userSettings.tabSize}
			wrapText={$userSettings.wrapText}
			diffFont={$userSettings.diffFont}
			diffLigatures={$userSettings.diffLigatures}
			diffContrast={$userSettings.diffContrast}
			colorBlindFriendly={$userSettings.colorBlindFriendly}
			inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
			hunkStr={diff}
		/>
	</CardGroup.Item>

	<CardGroup.Item>
		{#snippet title()}
			{$t('settings.general.appearance.diffFont.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.appearance.diffFont.caption')}
		{/snippet}

		<Textbox
			wide
			bind:value={$userSettings.diffFont}
			required
			onchange={(value: string) => {
				userSettings.update((s) => ({
					...s,
					diffFont: value
				}));
			}}
		/>
	</CardGroup.Item>

	<CardGroup.Item labelFor="allowDiffLigatures">
		{#snippet title()}
			{$t('settings.general.appearance.diffLigatures.title')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="allowDiffLigatures"
				checked={$userSettings.diffLigatures}
				onclick={() => {
					userSettings.update((s) => ({
						...s,
						diffLigatures: !$userSettings.diffLigatures
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item alignment="center">
		{#snippet title()}
			{$t('settings.general.appearance.tabSize.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.appearance.tabSize.caption')}
		{/snippet}

		{#snippet actions()}
			<Textbox
				type="number"
				width={100}
				textAlign="center"
				value={$userSettings.tabSize.toString()}
				minVal={1}
				maxVal={8}
				showCountActions
				onchange={(value: string) => {
					userSettings.update((s) => ({
						...s,
						tabSize: parseInt(value) || $userSettings.tabSize
					}));
				}}
				placeholder={$userSettings.tabSize.toString()}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="wrapText">
		{#snippet title()}
			{$t('settings.general.appearance.softWrap.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.appearance.softWrap.caption')}
		{/snippet}

		{#snippet actions()}
			<Toggle
				id="wrapText"
				checked={$userSettings.wrapText}
				onclick={() => {
					userSettings.update((s) => ({
						...s,
						wrapText: !s.wrapText
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item>
		{#snippet title()}
			{$t('settings.general.appearance.linesContrast.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.appearance.linesContrast.caption')}
		{/snippet}
		{#snippet actions()}
			<Select
				maxWidth={110}
				value={$userSettings.diffContrast}
				options={linesContrastOptions}
				onselect={(value) => {
					userSettings.update((s) => ({
						...s,
						diffContrast: value as 'strong' | 'medium' | 'light'
					}));
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === $userSettings.diffContrast} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="colorBlindFriendly">
		{#snippet title()}
			{$t('settings.general.appearance.colorBlindFriendly.title')}
		{/snippet}
		{#snippet caption()}
			{@html $t('settings.general.appearance.colorBlindFriendly.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="colorBlindFriendly"
				checked={$userSettings.colorBlindFriendly}
				onclick={() => {
					userSettings.update((s) => ({
						...s,
						colorBlindFriendly: !s.colorBlindFriendly
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="inlineUnifiedDiffs">
		{#snippet title()}
			{$t('settings.general.appearance.inlineWordDiffs.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.appearance.inlineWordDiffs.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="inlineUnifiedDiffs"
				checked={$userSettings.inlineUnifiedDiffs}
				onclick={() => {
					userSettings.update((s) => ({
						...s,
						inlineUnifiedDiffs: !s.inlineUnifiedDiffs
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<CardGroup>
	<form class="stack-v" onchange={(e) => onScrollbarFormChange(e.currentTarget)}>
		<CardGroup.Item labelFor="scrollbar-on-scroll">
			{#snippet title()}
				{$t('settings.general.appearance.scrollbarOnScroll.title')}
			{/snippet}
			{#snippet caption()}
				{$t('settings.general.appearance.scrollbarOnScroll.caption')}
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="scrollBarVisibilityType"
					value="scroll"
					id="scrollbar-on-scroll"
					checked={$userSettings.scrollbarVisibilityState === 'scroll'}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="scrollbar-on-hover">
			{#snippet title()}
				{$t('settings.general.appearance.scrollbarOnHover.title')}
			{/snippet}
			{#snippet caption()}
				{$t('settings.general.appearance.scrollbarOnHover.caption')}
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="scrollBarVisibilityType"
					value="hover"
					id="scrollbar-on-hover"
					checked={$userSettings.scrollbarVisibilityState === 'hover'}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="scrollbar-always">
			{#snippet title()}
				{$t('settings.general.appearance.scrollbarAlways.title')}
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="scrollBarVisibilityType"
					value="always"
					id="scrollbar-always"
					checked={$userSettings.scrollbarVisibilityState === 'always'}
				/>
			{/snippet}
		</CardGroup.Item>
	</form>
</CardGroup>

<CardGroup>
	<form class="stack-v" onchange={(e) => onStagingBehaviorFormChange(e.currentTarget)}>
		<CardGroup.Item labelFor="stage-all">
			{#snippet title()}
				{$t('settings.general.appearance.stagingBehavior.stageAll.title')}
			{/snippet}
			{#snippet caption()}
				{$t('settings.general.appearance.stagingBehavior.stageAll.caption')}
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
				{$t('settings.general.appearance.stagingBehavior.stageSelection.title')}
			{/snippet}
			{#snippet caption()}
				{@html $t('settings.general.appearance.stagingBehavior.stageSelection.caption')}
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
				{$t('settings.general.appearance.stagingBehavior.stageNone.title')}
			{/snippet}
			{#snippet caption()}
				{@html $t('settings.general.appearance.stagingBehavior.stageNone.caption')}
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
