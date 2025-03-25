<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { GitLabState } from '$lib/forge/gitlab/gitlabState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';

	const forge = getContext(DefaultForgeFactory);
	const gitLabState = getContext(GitLabState);
</script>

{#if forge.current.name === 'gitlab'}
	<Spacer />
	<SectionCard>
		<h3 class="text-bold text-15">Configure GitLab Integration</h3>
		<p class="text-13">
			Learn how find your GitLab Personal Token and Project ID in our <Link
				href="https://docs.gitbutler.com/features/gitlab-integration">documentation</Link
			>
		</p>

		<Textbox label="Personal Token" bind:value={gitLabState.token.current} />
		<Textbox label="Project ID" bind:value={gitLabState.gitlabProjectId.current} />
		<Textbox label="Instance URL" bind:value={gitLabState.instanceUrl.current} />
	</SectionCard>
{/if}
