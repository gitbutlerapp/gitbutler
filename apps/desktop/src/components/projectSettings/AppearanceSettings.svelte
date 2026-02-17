<script lang="ts">
	import ThemeSelector from "$components/ThemeSelector.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { inject } from "@gitbutler/core/context";
	import {
		CardGroup,
		HunkDiff,
		RadioButton,
		Select,
		SelectItem,
		Textbox,
		Toggle,
	} from "@gitbutler/ui";
	import type { ScrollbarVisilitySettings } from "@gitbutler/ui/components/scroll/Scrollbar.svelte";

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
			"scrollBarVisibilityType",
		) as ScrollbarVisilitySettings;

		userSettings.update((s) => ({
			...s,
			scrollbarVisibilityState: selectedScrollbarVisibility,
		}));
	}
</script>

<CardGroup.Item standalone>
	{#snippet title()}
		Theme
	{/snippet}
	<ThemeSelector {userSettings} />
</CardGroup.Item>

<CardGroup.Item alignment="center" standalone>
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
				{ label: "List view", value: "list" },
				{ label: "Tree view", value: "tree" },
			]}
			onselect={(value) => {
				userSettings.update((s) => ({
					...s,
					defaultFileListMode: value as "tree" | "list",
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
		File path first
	{/snippet}
	{#snippet caption()}
		Display the full file path before the file name in file lists.
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="pathFirst"
			checked={$userSettings.pathFirst}
			onclick={() => {
				userSettings.update((s) => ({
					...s,
					pathFirst: !s.pathFirst,
				}));
			}}
		/>
	{/snippet}
</CardGroup.Item>

<CardGroup.Item labelFor="singleDiffView" standalone>
	{#snippet title()}
		Single diff view
	{/snippet}
	{#snippet caption()}
		Show only the selected file's diff instead of a scrollable list of all file diffs.
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="singleDiffView"
			checked={$userSettings.singleDiffView}
			onclick={() => {
				userSettings.update((s) => ({
					...s,
					singleDiffView: !s.singleDiffView,
				}));
			}}
		/>
	{/snippet}
</CardGroup.Item>

<CardGroup>
	<CardGroup.Item alignment="center">
		{#snippet title()}
			Diff preview
		{/snippet}

		<HunkDiff
			filePath="test.tsx"
			tabSize={$userSettings.tabSize}
			wrapText={$userSettings.wrapText}
			diffFont={$userSettings.diffFont}
			diffLigatures={$userSettings.diffLigatures}
			strongContrast={$userSettings.strongContrast}
			colorBlindFriendly={$userSettings.colorBlindFriendly}
			inlineUnifiedDiffs={$userSettings.inlineUnifiedDiffs}
			hunkStr={diff}
		/>
	</CardGroup.Item>

	<CardGroup.Item>
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
					diffFont: value,
				}));
			}}
		/>
	</CardGroup.Item>

	<CardGroup.Item labelFor="allowDiffLigatures">
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
						diffLigatures: !$userSettings.diffLigatures,
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item alignment="center">
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
						tabSize: parseInt(value) || $userSettings.tabSize,
					}));
				}}
				placeholder={$userSettings.tabSize.toString()}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="wrapText">
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
						wrapText: !s.wrapText,
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="strongContrast">
		{#snippet title()}
			Strong contrast
		{/snippet}
		{#snippet caption()}
			Use stronger contrast for added, deleted, and context lines in diffs.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="strongContrast"
				checked={$userSettings.strongContrast}
				onclick={() => {
					userSettings.update((s) => ({
						...s,
						strongContrast: !s.strongContrast,
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="colorBlindFriendly">
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
						colorBlindFriendly: !s.colorBlindFriendly,
					}));
				}}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="inlineUnifiedDiffs">
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
						inlineUnifiedDiffs: !s.inlineUnifiedDiffs,
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
					checked={$userSettings.scrollbarVisibilityState === "scroll"}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="scrollbar-on-hover">
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
					checked={$userSettings.scrollbarVisibilityState === "hover"}
				/>
			{/snippet}
		</CardGroup.Item>

		<CardGroup.Item labelFor="scrollbar-always">
			{#snippet title()}
				Always show scrollbar
			{/snippet}
			{#snippet actions()}
				<RadioButton
					name="scrollBarVisibilityType"
					value="always"
					id="scrollbar-always"
					checked={$userSettings.scrollbarVisibilityState === "always"}
				/>
			{/snippet}
		</CardGroup.Item>
	</form>
</CardGroup>
