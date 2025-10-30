<script lang="ts">
	import { Badge } from '@gitbutler/ui';
	import type { GitHubAccountIdentifier } from '$lib/forge/github/githubUserService.svelte';

	type Props = {
		account: GitHubAccountIdentifier;
		class?: string;
	};

	const { account, class: className }: Props = $props();

	const tooltipText = $derived(account.type === 'enterprise' ? 'GitHub Enterprise' : account.type);
	const badgeText = $derived(account.type === 'enterprise' ? account.info.host : account.type);
</script>

<Badge class={className} tooltip={tooltipText}>
	{badgeText}
</Badge>
