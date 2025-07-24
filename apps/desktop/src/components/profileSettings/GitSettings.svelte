<script lang="ts">
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { inject } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { onMount } from 'svelte';

	const gitConfig = inject(GIT_CONFIG_SERVICE);

	let annotateCommits = $state(true);

	function toggleCommitterSigning() {
		annotateCommits = !annotateCommits;
		gitConfig.set('gitbutler.gitbutlerCommitter', annotateCommits ? '1' : '0');
	}

	onMount(async () => {
		annotateCommits = (await gitConfig.get('gitbutler.gitbutlerCommitter')) === '1';
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
			target="_blank"
			rel="noreferrer"
			href="https://github.com/gitbutlerapp/gitbutler-docs/blob/d81a23779302c55f8b20c75bf7842082815b4702/content/docs/features/virtual-branches/committer-mark.mdx"
		>
			Learn more
		</Link>
	{/snippet}
	{#snippet actions()}
		<Toggle id="committerSigning" checked={annotateCommits} onclick={toggleCommitterSigning} />
	{/snippet}
</SectionCard>
