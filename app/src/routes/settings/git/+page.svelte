<script lang="ts">
	import { AuthService } from '$lib/backend/auth';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import ContentWrapper from '$lib/settings/ContentWrapper.svelte';
	import Button from '$lib/shared/Button.svelte';
	import Link from '$lib/shared/Link.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { getContext } from '$lib/utils/context';
	import { openExternalUrl } from '$lib/utils/url';
	import { onMount } from 'svelte';

	const gitConfig = getContext(GitConfigService);
	const authService = getContext(AuthService);

	let annotateCommits = true;
	let sshKey = '';

	function toggleCommitterSigning() {
		annotateCommits = !annotateCommits;
		gitConfig.set('gitbutler.gitbutlerCommitter', annotateCommits ? '1' : '0');
	}

	onMount(async () => {
		sshKey = await authService.getPublicKey();
		annotateCommits = (await gitConfig.get('gitbutler.gitbutlerCommitter')) === '1';
	});
</script>

<ContentWrapper title="Git stuff">
	<SectionCard labelFor="committerSigning" orientation="row">
		<svelte:fragment slot="title">Credit GitButler as the committer</svelte:fragment>
		<svelte:fragment slot="caption">
			By default, everything in the GitButler client is free to use. You can opt in to crediting us
			as the committer in your virtual branch commits to help spread the word.
			<Link
				target="_blank"
				rel="noreferrer"
				href="https://docs.gitbutler.com/features/virtual-branches/committer-mark"
			>
				Learn more
			</Link>
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="committerSigning" checked={annotateCommits} on:click={toggleCommitterSigning} />
		</svelte:fragment>
	</SectionCard>

	<Spacer />

	<SectionCard>
		<svelte:fragment slot="title">SSH key</svelte:fragment>
		<svelte:fragment slot="caption">
			GitButler uses SSH keys to authenticate with your Git provider. Add the following public key
			to your Git provider to enable GitButler to push code.
		</svelte:fragment>

		<TextBox readonly selectall bind:value={sshKey} />
		<div class="row-buttons">
			<Button style="pop" kind="solid" icon="copy" on:click={() => copyToClipboard(sshKey)}>
				Copy to clipboard
			</Button>
			<Button
				style="ghost"
				outline
				icon="open-link"
				on:mousedown={() => {
					openExternalUrl('https://github.com/settings/ssh/new');
				}}
			>
				Add key to GitHub
			</Button>
		</div>
	</SectionCard>
</ContentWrapper>
