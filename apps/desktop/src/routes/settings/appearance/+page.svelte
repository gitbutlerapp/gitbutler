<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { autoSelectBranchNameFeature } from '$lib/config/uiFeatureFlags';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import ThemeSelector from '$lib/settings/ThemeSelector.svelte';
	import {
		SETTINGS,
		type Settings,
		type ScrollbarVisilitySettings
	} from '$lib/settings/userSettings';
	import RadioButton from '$lib/shared/RadioButton.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getContextStoreBySymbol } from '$lib/utils/context';
	import type { Writable } from 'svelte/store';

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

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

	<form onchange={(e) => onScrollbarFormChange(e.currentTarget)}>
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
		<svelte:fragment slot="title">Auto-highlight Branch Lane Contents</svelte:fragment>
		<svelte:fragment slot="caption">
			An experimental UI toggle to highlight the contents of the branch lane input fields when
			clicking into them.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="branchLaneContents"
				checked={$autoSelectBranchNameFeature}
				on:click={() => ($autoSelectBranchNameFeature = !$autoSelectBranchNameFeature)}
			/>
		</svelte:fragment>
	</SectionCard>
</SettingsPage>
