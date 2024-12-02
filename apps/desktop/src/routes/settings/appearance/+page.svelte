<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { autoSelectBranchNameFeature } from '$lib/config/uiFeatureFlags';
	import HunkDiff from '$lib/hunk/HunkDiff.svelte';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import ThemeSelector from '$lib/settings/ThemeSelector.svelte';
	import {
		SETTINGS,
		type Settings,
		type ScrollbarVisilitySettings,
		type CodeEditorSettings
	} from '$lib/settings/userSettings';
	import RadioButton from '$lib/shared/RadioButton.svelte';
	import { type Hunk } from '$lib/vbranches/types';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import type { ContentSection } from '$lib/utils/fileSections';
	import type { Writable } from 'svelte/store';

	const editorOptions: CodeEditorSettings[] = [
		{ schemeIdentifer: 'vscodium', displayName: 'VSCodium' },
		{ schemeIdentifer: 'vscode', displayName: 'VSCode' },
		{ schemeIdentifer: 'vscode-insiders', displayName: 'VSCode Insiders' },
		{ schemeIdentifer: 'windsurf', displayName: 'Windsurf' },
		{ schemeIdentifer: 'zed', displayName: 'Zed' },
		{ schemeIdentifer: 'cursor', displayName: 'Cursor' }
	];
	const editorOptionsForSelect = editorOptions.map((option) => ({
		label: option.displayName,
		value: option.schemeIdentifer
	}));

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	const testHunk: Hunk = {
		id: '59-66',
		hash: 'test',
		modifiedAt: new Date(),
		lockedTo: [],
		locked: false,
		poisoned: false,
		changeType: 'modified',
		diff: '',
		filePath: 'test',
		new_start: 59,
		new_lines: 7
	};

	// prettier-ignore
	const hunkSubsections: ContentSection[] = [
		{expanded: true, lines: [{beforeLineNumber: 56, afterLineNumber: 56, content: "\t\t\t// Diff example"}], sectionType: 2, maxLineNumber: 55},
		{expanded: true, lines: [{beforeLineNumber: 57, afterLineNumber: 57, content: "\t\t\tprojectName={project.title}"}, {beforeLineNumber: 58, afterLineNumber: 58, content: "\t\t\t{remoteBranches}"}, {beforeLineNumber: 59, afterLineNumber: 59, content: "\t\t\ton:branchSelected={async (e) => {"}], sectionType: 2, maxLineNumber: 59},
		{expanded: true, lines: [{beforeLineNumber: 61, afterLineNumber: undefined, content: "\t\t\t\tselectedBranch = e.detail;"}], sectionType: 2, maxLineNumber: 60},
		{expanded: true, lines: [{beforeLineNumber: 62, afterLineNumber: undefined, content: "\t\t\t\tif ($platformName === 'win32') {"}], sectionType: 1, maxLineNumber: 61},
		{expanded: true, lines: [{beforeLineNumber: undefined, afterLineNumber: 61, content: "\t\t\t\tif ($platformName === 'win64') {"}], sectionType: 0, maxLineNumber: 61},
		{expanded: true, lines: [{beforeLineNumber: 63, afterLineNumber: 62, content: "\t\t\t\t\tsetTarget();"}, {beforeLineNumber: 64, afterLineNumber: 63, content: "\t\t\t\t}"}, {beforeLineNumber: 65, afterLineNumber: 64, content: "\t\t\t}}"}], sectionType: 2, maxLineNumber: 65}
	];

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
</script>

<SettingsPage title="Appearance">
	<SectionCard>
		{#snippet title()}
			Theme
		{/snippet}
		<ThemeSelector {userSettings} />
	</SectionCard>
	<SectionCard orientation="row" centerAlign>
		{#snippet title()}
			Default code editor
		{/snippet}
		{#snippet actions()}
			<Select
				value={$userSettings.defaultCodeEditor.schemeIdentifer}
				options={editorOptionsForSelect}
				onselect={(value) => {
					const selected = editorOptions.find((option) => option.schemeIdentifer === value);
					if (selected) {
						userSettings.update((s) => ({ ...s, defaultCodeEditor: selected }));
					}
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem
						selected={item.value === $userSettings.defaultCodeEditor.schemeIdentifer}
						{highlighted}
					>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/snippet}
	</SectionCard>
	<div class="stack-v">
		<SectionCard centerAlign roundedBottom={false}>
			{#snippet title()}
				Diff preview
			{/snippet}

			<HunkDiff
				readonly
				filePath="test.tsx"
				minWidth={1.25}
				selectable={false}
				draggingDisabled
				tabSize={$userSettings.tabSize}
				wrapText={$userSettings.wrapText}
				diffFont={$userSettings.diffFont}
				diffLigatures={$userSettings.diffLigatures}
				inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
				hunk={testHunk}
				subsections={hunkSubsections}
				onclick={() => {}}
				handleSelected={() => {}}
				handleLineContextMenu={() => {}}
			/>
		</SectionCard>

		<SectionCard orientation="column" roundedTop={false} roundedBottom={false}>
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

		<SectionCard orientation="row" centerAlign roundedTop={false} roundedBottom={false}>
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

		<SectionCard labelFor="wrapText" orientation="row" roundedTop={false} roundedBottom={false}>
			{#snippet title()}
				Text wrap
			{/snippet}
			{#snippet caption()}
				Wrap text in the diff view once it hits the end of the viewport.
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

		<SectionCard labelFor="inlineUnifiedDiffs" orientation="row" roundedTop={false}>
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
		<SectionCard roundedBottom={false} orientation="row" labelFor="scrollbar-on-scroll">
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

		<SectionCard roundedTop={false} orientation="row" labelFor="scrollbar-always">
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

	<SectionCard labelFor="branchLaneContents" orientation="row">
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
</SettingsPage>
