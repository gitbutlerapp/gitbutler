<script lang="ts">
	import { Tooltip } from '@gitbutler/ui';
	import type { GitHubAccountIdentifierType } from '$lib/forge/github/githubUserService.svelte';

	type Props = {
		type: GitHubAccountIdentifierType;
	};

	const { type }: Props = $props();
</script>

{#if type === 'enterprise'}
	<Tooltip text="This is a GitHub Enterprise account">
		<span class="text-11 pill enterprise">Enterprise account</span>
	</Tooltip>
{:else if type === 'patUsername'}
	<Tooltip text="This account was added using a Personal Access Token">
		<span class="text-11 pill pat">PAT</span>
	</Tooltip>
{/if}

<style>
	.pill {
		padding: 2px 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: 12px;
		background-color: var(--clr-bg-2);
	}
</style>
