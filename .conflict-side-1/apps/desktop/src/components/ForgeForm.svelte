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
	const token = gitLabState.token;
	const forkProjectId = gitLabState.forkProjectId;
	const upstreamProjectId = gitLabState.upstreamProjectId;
	const instanceUrl = gitLabState.instanceUrl;
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
		<p class="text-13">
			The Fork Project ID should be the project that your branches get pushed to, and the Upstream
			Project ID should be project where you want the Merge Requests to be created.
		</p>

		<Textbox label="Personal Token" value={$token} oninput={(value) => ($token = value)} />
		<Textbox
			label="Your Fork's Project ID"
			value={$forkProjectId}
			oninput={(value) => ($forkProjectId = value)}
		/>
		<Textbox
			label="Upstream Project ID"
			value={$upstreamProjectId}
			oninput={(value) => ($upstreamProjectId = value)}
		/>
		<Textbox
			label="Instance URL"
			value={$instanceUrl}
			oninput={(value) => ($instanceUrl = value)}
		/>
		<p class="text-13">
			If you use a custom GitLab instance (not gitlab.com), you will need to add it as a custom CSP
			entry so that GitButler trusts connecting to that host. Read more in the <Link
				href="https://docs.gitbutler.com/troubleshooting/custom-csp">docs</Link
			>
		</p>
	</SectionCard>
{/if}
