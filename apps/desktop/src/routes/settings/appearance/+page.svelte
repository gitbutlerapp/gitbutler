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
	import TextBox from '$lib/shared/TextBox.svelte';
	import { type Hunk } from '$lib/vbranches/types';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import type { ContentSection } from '$lib/utils/fileSections';
	import type { Writable } from 'svelte/store';

	const editorOptions: CodeEditorSettings[] = [
		{ schemeIdentifer: 'vscodium', displayName: 'VSCodium' },
		{ schemeIdentifer: 'vscode', displayName: 'VSCode' },
		{ schemeIdentifer: 'vscode-insiders', displayName: 'VSCode Insiders' },
		{ schemeIdentifer: 'zed', displayName: 'Zed' }
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
		<svelte:fragment slot="title">Theme</svelte:fragment>
		<ThemeSelector {userSettings} />
	</SectionCard>
	<SectionCard orientation="row" centerAlign>
		<svelte:fragment slot="title">Default code editor</svelte:fragment>
		<svelte:fragment slot="actions">
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
		</svelte:fragment>
	</SectionCard>
	<div class="stack-v">
		<SectionCard centerAlign roundedBottom={false}>
			<svelte:fragment slot="title">Diff preview</svelte:fragment>

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
			<svelte:fragment slot="title">Font family</svelte:fragment>
			<svelte:fragment slot="caption"
				>Sets the font for the diff view. The first font name is the default, others are fallbacks.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<TextBox
					wide
					bind:value={$userSettings.diffFont}
					required
					on:change={(e) => {
						userSettings.update((s) => ({
							...s,
							diffFont: e.detail
						}));
					}}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard
			labelFor="allowDiffLigatures"
			orientation="row"
			roundedTop={false}
			roundedBottom={false}
		>
			<svelte:fragment slot="title">Allow font ligatures</svelte:fragment>
			<svelte:fragment slot="actions">
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
			</svelte:fragment>
		</SectionCard>

		<SectionCard orientation="row" centerAlign roundedTop={false} roundedBottom={false}>
			<svelte:fragment slot="title">Tab size</svelte:fragment>
			<svelte:fragment slot="caption">
				The number of spaces a tab is equal to when previewing code changes.
			</svelte:fragment>

			<svelte:fragment slot="actions">
				<TextBox
					type="number"
					width={100}
					textAlign="center"
					value={$userSettings.tabSize.toString()}
					minVal={1}
					maxVal={8}
					showCountActions
					on:change={(e) => {
						userSettings.update((s) => ({
							...s,
							tabSize: parseInt(e.detail) || $userSettings.tabSize
						}));
					}}
					placeholder={$userSettings.tabSize.toString()}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard labelFor="wrapText" orientation="row" roundedTop={false} roundedBottom={false}>
			<svelte:fragment slot="title">Text Wrap</svelte:fragment>
			<svelte:fragment slot="caption">
				Wrap text in the diff view once it hits the end of the viewport.
			</svelte:fragment>

			<svelte:fragment slot="actions">
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
			</svelte:fragment>
		</SectionCard>

		<SectionCard labelFor="inlineUnifiedDiffs" orientation="row" roundedTop={false}>
			<svelte:fragment slot="title">Display word diffs inline</svelte:fragment>
			<svelte:fragment slot="caption">
				Instead of separate lines for removals and additions, this feature shows a single line with
				both added and removed words highlighted.
			</svelte:fragment>
			<svelte:fragment slot="actions">
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
			</svelte:fragment>
		</SectionCard>
	</div>

	<form class="stack-v" on:change={(e) => onScrollbarFormChange(e.currentTarget)}>
		<SectionCard roundedBottom={false} orientation="row" labelFor="scrollbar-on-scroll">
			<svelte:fragment slot="title">Scrollbar-On-Scroll</svelte:fragment>
			<svelte:fragment slot="caption">
				Only show the scrollbar when you are scrolling.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton
					name="scrollBarVisibilityType"
					value="scroll"
					id="scrollbar-on-scroll"
					checked={$userSettings.scrollbarVisibilityState === 'scroll'}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard
			roundedTop={false}
			roundedBottom={false}
			orientation="row"
			labelFor="scrollbar-on-hover"
		>
			<svelte:fragment slot="title">Scrollbar-On-Hover</svelte:fragment>
			<svelte:fragment slot="caption">
				Show the scrollbar only when you hover over the scrollable area.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton
					name="scrollBarVisibilityType"
					value="hover"
					id="scrollbar-on-hover"
					checked={$userSettings.scrollbarVisibilityState === 'hover'}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard roundedTop={false} orientation="row" labelFor="scrollbar-always">
			<svelte:fragment slot="title">Always show scrollbar</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton
					name="scrollBarVisibilityType"
					value="always"
					id="scrollbar-always"
					checked={$userSettings.scrollbarVisibilityState === 'always'}
				/>
			</svelte:fragment>
		</SectionCard>
	</form>

	<SectionCard labelFor="branchLaneContents" orientation="row">
		<svelte:fragment slot="title">Auto-select text on branch/lane rename</svelte:fragment>
		<svelte:fragment slot="caption">
			Enable this option to automatically select the text when the input is focused.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="branchLaneContents"
				checked={$autoSelectBranchNameFeature}
				onclick={() => ($autoSelectBranchNameFeature = !$autoSelectBranchNameFeature)}
			/>
		</svelte:fragment>
	</SectionCard>
</SettingsPage>
