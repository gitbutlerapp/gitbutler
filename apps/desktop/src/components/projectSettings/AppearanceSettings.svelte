<script lang="ts">
	import ThemeSelector from '$components/ThemeSelector.svelte';
	import { stagingBehaviorFeature, type StagingBehavior } from '$lib/config/uiFeatureFlags';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import {
		HunkDiff,
		RadioButton,
		Section,
		Select,
		SelectItem,
		Textbox,
		Toggle
	} from '@gitbutler/ui';
	import type { ScrollbarVisilitySettings } from '@gitbutler/ui/components/scroll/Scrollbar.svelte';

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
</script>

<Section.Card standalone>
	{#snippet title()}
		Theme
	{/snippet}
	<ThemeSelector {userSettings} />
</Section.Card>

<Section.Card alignment="center" standalone>
	{#snippet title()}
		Default file list mode
	{/snippet}
	{#snippet caption()}
		Set the default file list view (can be changed per location).
	{/snippet}
	{#snippet actions()}
		<Select
			maxWidth={120}
			value={$userSettings.defaultFileListMode}
			options={[
				{ label: 'List view', value: 'list' },
				{ label: 'Tree view', value: 'tree' }
			]}
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
</Section.Card>

<Section>
	<Section.Card alignment="center">
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
			colorBlindFriendly={$userSettings.colorBlindFriendly}
			inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
			hunkStr={diff}
		/>
	</Section.Card>

	<Section.Card>
		{#snippet title()}
			Font family
		{/snippet}
		{#snippet caption()}
			Sets the font for the diff view. The first font name is the default, others are fallbacks.
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
	</Section.Card>

	<Section.Card labelFor="allowDiffLigatures">
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
	</Section.Card>

	<Section.Card alignment="center">
		{#snippet title()}
			Tab size
		{/snippet}
		{#snippet caption()}
			Number of spaces per tab in the diff view.
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
	</Section.Card>

	<Section.Card labelFor="wrapText">
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
	</Section.Card>

	<Section.Card>
		{#snippet title()}
			Lines contrast
		{/snippet}
		{#snippet caption()}
			The contrast for added, deleted, and context lines in diffs.
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
	</Section.Card>

	<Section.Card labelFor="colorBlindFriendly">
		{#snippet title()}
			Color blind-friendly colors
		{/snippet}
		{#snippet caption()}
			Use blue and orange colors instead of green and red for better
			<br />
			accessibility with color vision deficiency.
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
	</Section.Card>

	<Section.Card labelFor="inlineUnifiedDiffs">
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
	</Section.Card>
</Section>

<Section>
	<form class="stack-v" onchange={(e) => onScrollbarFormChange(e.currentTarget)}>
		<Section.Card labelFor="scrollbar-on-scroll">
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
		</Section.Card>

		<Section.Card labelFor="scrollbar-on-hover">
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
		</Section.Card>

		<Section.Card labelFor="scrollbar-always">
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
		</Section.Card>
	</form>
</Section>

<Section>
	<form class="stack-v" onchange={(e) => onStagingBehaviorFormChange(e.currentTarget)}>
		<Section.Card labelFor="stage-all">
			{#snippet title()}
				Stage all files
			{/snippet}
			{#snippet caption()}
				Stage all files assigned to the stack on commit. If no files are staged, all unassinged
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
		</Section.Card>

		<Section.Card labelFor="stage-selection">
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
		</Section.Card>

		<Section.Card labelFor="stage-none">
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
		</Section.Card>
	</form>
</Section>
