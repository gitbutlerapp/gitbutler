<script lang="ts">
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Select, SelectItem, Toggle } from '@gitbutler/ui';
	import { onMount } from 'svelte';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const gitConfig = inject(GIT_CONFIG_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settings = settingsService.appSettings;

	let annotateCommits = $state(true);
	let fetchFrequency = $state<number>(-1);

	const fetchFrequencyOptions = $derived([
		{ label: $t('settings.general.git.autoFetch.oneMinute'), value: '1', minutes: 1 },
		{ label: $t('settings.general.git.autoFetch.fiveMinutes'), value: '5', minutes: 5 },
		{ label: $t('settings.general.git.autoFetch.tenMinutes'), value: '10', minutes: 10 },
		{ label: $t('settings.general.git.autoFetch.fifteenMinutes'), value: '15', minutes: 15 },
		{ label: $t('settings.general.git.autoFetch.none'), value: 'none', minutes: -1 }
	] as const);

	function toggleCommitterSigning() {
		annotateCommits = !annotateCommits;
		gitConfig.set('gitbutler.gitbutlerCommitter', annotateCommits ? '1' : '0');
	}

	async function updateFetchFrequency(value: string) {
		const option = fetchFrequencyOptions.find((opt) => opt.value === value);
		if (option) {
			fetchFrequency = option.minutes;
			await settingsService.updateFetch({ autoFetchIntervalMinutes: option.minutes });
		}
	}

	const selectedValue = $derived(
		fetchFrequencyOptions.find((opt) => opt.minutes === fetchFrequency)?.value ?? 'none'
	);

	onMount(async () => {
		annotateCommits = (await gitConfig.get('gitbutler.gitbutlerCommitter')) === '1';
	});

	$effect(() => {
		if ($settings?.fetch) {
			fetchFrequency = $settings.fetch.autoFetchIntervalMinutes;
		}
	});
</script>

<CardGroup.Item standalone labelFor="committerSigning">
	{#snippet title()}
		{$t('settings.general.git.committerCredit.title')}
	{/snippet}
	{#snippet caption()}
		{@html $t('settings.general.git.committerCredit.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle id="committerSigning" checked={annotateCommits} onclick={toggleCommitterSigning} />
	{/snippet}
</CardGroup.Item>

<CardGroup.Item standalone labelFor="fetchFrequency" alignment="center">
	{#snippet title()}
		{$t('settings.general.git.autoFetch.title')}
	{/snippet}
	{#snippet actions()}
		<Select
			id="fetchFrequency"
			options={fetchFrequencyOptions}
			value={selectedValue}
			onselect={updateFetchFrequency}
		>
			{#snippet itemSnippet({ item, highlighted })}
				<SelectItem selected={item.value === selectedValue} {highlighted}>
					{item.label}
				</SelectItem>
			{/snippet}
		</Select>
	{/snippet}
</CardGroup.Item>
