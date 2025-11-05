<script lang="ts">
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { inject } from '@gitbutler/core/context';
	import { Link, SectionCard, Toggle, Select, SelectItem } from '@gitbutler/ui';
	import { onMount } from 'svelte';

	const gitConfig = inject(GIT_CONFIG_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settings = settingsService.appSettings;

	let annotateCommits = $state(true);
	let fetchFrequency = $state<number>(-1);

	const fetchFrequencyOptions = [
		{ label: '1 minute', value: '1', minutes: 1 },
		{ label: '5 minutes', value: '5', minutes: 5 },
		{ label: '10 minutes', value: '10', minutes: 10 },
		{ label: '15 minutes', value: '15', minutes: 15 },
		{ label: 'None', value: 'none', minutes: -1 }
	] as const;

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

<SectionCard labelFor="committerSigning" orientation="row">
	{#snippet title()}
		Credit GitButler as the committer
	{/snippet}
	{#snippet caption()}
		By default, everything in the GitButler client is free to use. You can opt in to crediting us as
		the committer in your virtual branch commits to help spread the word.
		<Link
			href="https://github.com/gitbutlerapp/gitbutler-docs/blob/d81a23779302c55f8b20c75bf7842082815b4702/content/docs/features/virtual-branches/committer-mark.mdx"
		>
			Learn more
		</Link>
	{/snippet}
	{#snippet actions()}
		<Toggle id="committerSigning" checked={annotateCommits} onclick={toggleCommitterSigning} />
	{/snippet}
</SectionCard>

<SectionCard labelFor="fetchFrequency" orientation="row" centerAlign>
	{#snippet title()}
		Auto-fetch frequency
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
</SectionCard>
