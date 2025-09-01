<script lang="ts">
	import ThemeSelector from '$components/ThemeSelector.svelte';
	import {
		autoSelectBranchNameFeature,
		stagingBehaviorFeature,
		type StagingBehavior
	} from '$lib/config/uiFeatureFlags';
	import { focusable } from '$lib/focus/focusable';
	import { SETTINGS, type ScrollbarVisilitySettings } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import {
		HunkDiff,
		RadioButton,
		SectionCard,
		Select,
		SelectItem,
		Textbox,
		Toggle
	} from '@gitbutler/ui';

	const userSettings = inject(SETTINGS);
	const diff = `@@ -56,10 +56,9 @@
			// Diff example
			projectName={project.title}
			{remoteBranches}
			on:branchSelected={async (e) => {
-				selectedBranch = e.detail;
-				if ($platformName === 'win32') {
+				if ($platformName === 'win64' && $userSettings.enableAdvancedFeatures && project.hasRemoteOrigin) {
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
</script>

<SectionCard {focusable}>
	{#snippet title()}
		Theme
	{/snippet}
	<ThemeSelector {userSettings} />
</SectionCard>
<div class="stack-v">
	<SectionCard centerAlign roundedBottom={false} {focusable}>
		{#snippet title()}
			Diff preview
		{/snippet}

		<HunkDiff
			filePath="test.tsx"
			tabSize={$userSettings.tabSize}
			wrapText={$userSettings.wrapText}
			diffFont={$userSettings.diffFont}
			diffLigatures={$userSettings.diffLigatures}
			diffContrast={$userSettings.diffContrast}
			inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
			hunkStr={diff}
		/>
	</SectionCard>

	<SectionCard orientation="column" roundedTop={false} roundedBottom={false} {focusable}>
		{#snippet title()}
			Font family
		{/snippet}
		{#snippet caption()}
			Sets the font for the diff view. The first font name is the default, others are fallbacks.
		{/snippet}
		{#snippet actions()}
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
		{/snippet}
	</SectionCard>

	<SectionCard
		labelFor="allowDiffLigatures"
		orientation="row"
		roundedTop={false}
		roundedBottom={false}
		{focusable}
	>
		{#snippet title()}
			Allow font ligatures
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
	</SectionCard>

	<SectionCard orientation="row" centerAlign roundedTop={false} roundedBottom={false} {focusable}>
		{#snippet title()}
			Tab size
		{/snippet}
		{#snippet caption()}
			The number of spaces a tab is equal to when previewing code changes.
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
	</SectionCard>

	<SectionCard
		labelFor="wrapText"
		orientation="row"
		roundedTop={false}
		roundedBottom={false}
		{focusable}
	>
		{#snippet title()}
			Soft wrap
		{/snippet}
		{#snippet caption()}
			Soft wrap long lines in the diff view to fit within the viewport.
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
	</SectionCard>

	<SectionCard orientation="row" roundedTop={false} roundedBottom={false} {focusable}>
		{#snippet title()}
			Lines contrast
		{/snippet}
		{#snippet caption()}
			The contrast level of the diff lines — added, deleted, and counter lines.
		{/snippet}
		{#snippet actions()}
			<Select
				maxWidth={110}
				value={$userSettings.diffContrast}
				options={[
					{ label: 'Light', value: 'light' },
					{ label: 'Medium', value: 'medium' },
					{ label: 'Strong', value: 'strong' }
				]}
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
	</SectionCard>

	<SectionCard labelFor="inlineUnifiedDiffs" orientation="row" roundedTop={false} {focusable}>
		{#snippet title()}
			Display word diffs inline
		{/snippet}
		{#snippet caption()}
			Instead of separate lines for removals and additions, this feature shows a single line with
			both added and removed words highlighted.
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
	</SectionCard>
</div>

<form class="stack-v" onchange={(e) => onScrollbarFormChange(e.currentTarget)}>
	<SectionCard roundedBottom={false} orientation="row" labelFor="scrollbar-on-scroll" {focusable}>
		{#snippet title()}
			Scrollbar-On-Scroll
		{/snippet}
		{#snippet caption()}
			Only show the scrollbar when you are scrolling.
		{/snippet}
		{#snippet actions()}
			<RadioButton
				name="scrollBarVisibilityType"
				value="scroll"
				id="scrollbar-on-scroll"
				checked={$userSettings.scrollbarVisibilityState === 'scroll'}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard
		roundedTop={false}
		roundedBottom={false}
		orientation="row"
		labelFor="scrollbar-on-hover"
		{focusable}
	>
		{#snippet title()}
			Scrollbar-On-Hover
		{/snippet}
		{#snippet caption()}
			Show the scrollbar only when you hover over the scrollable area.
		{/snippet}
		{#snippet actions()}
			<RadioButton
				name="scrollBarVisibilityType"
				value="hover"
				id="scrollbar-on-hover"
				checked={$userSettings.scrollbarVisibilityState === 'hover'}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard roundedTop={false} orientation="row" labelFor="scrollbar-always" {focusable}>
		{#snippet title()}
			Always show scrollbar
		{/snippet}
		{#snippet actions()}
			<RadioButton
				name="scrollBarVisibilityType"
				value="always"
				id="scrollbar-always"
				checked={$userSettings.scrollbarVisibilityState === 'always'}
			/>
		{/snippet}
	</SectionCard>
</form>

<SectionCard labelFor="branchLaneContents" orientation="row" {focusable}>
	{#snippet title()}
		Auto-select text on branch/lane rename
	{/snippet}
	{#snippet caption()}
		Enable this option to automatically select the text when the input is focused.
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="branchLaneContents"
			checked={$autoSelectBranchNameFeature}
			onclick={() => ($autoSelectBranchNameFeature = !$autoSelectBranchNameFeature)}
		/>
	{/snippet}
</SectionCard>

<form class="stack-v" onchange={(e) => onStagingBehaviorFormChange(e.currentTarget)}>
	<SectionCard roundedBottom={false} orientation="row" labelFor="stage-all" {focusable}>
		{#snippet title()}
			Stage all files
		{/snippet}
		{#snippet caption()}
			Stage all files assigned to the stack on commit. If no files are staged, all unassinged files
			will be staged.
		{/snippet}
		{#snippet actions()}
			<RadioButton
				name="stagingBehaviorType"
				value="all"
				id="stage-all"
				checked={$stagingBehaviorFeature === 'all'}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard
		roundedTop={false}
		roundedBottom={false}
		orientation="row"
		labelFor="stage-selection"
		{focusable}
	>
		{#snippet title()}
			Stage selected files
		{/snippet}
		{#snippet caption()}
			Stage the selected assigned files to the stack on commit. If no files are selected, stage all
			files. If there are no assigned files, stage all selected unassigned files. And if no files
			are selected, stage all unassigned files.
		{/snippet}
		{#snippet actions()}
			<RadioButton
				name="stagingBehaviorType"
				value="selection"
				id="stage-selection"
				checked={$stagingBehaviorFeature === 'selection'}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard roundedTop={false} orientation="row" labelFor="stage-none" {focusable}>
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
	</SectionCard>
</form>
