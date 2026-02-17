<script lang="ts">
	import { Badge } from "@gitbutler/ui";
	import type { ButGitHubToken } from "@gitbutler/core/api";

	type Props = {
		account: ButGitHubToken.GithubAccountIdentifier;
		class?: string;
	};

	const { account, class: className }: Props = $props();

	// const tooltipText = $derived(account.type === 'enterprise' ? 'GitHub Enterprise' : account.type);
	// const badgeText = $derived(account.type === 'enterprise' ? account.info.host : account.type);
	export function badgeText(account: ButGitHubToken.GithubAccountIdentifier): string | null {
		switch (account.type) {
			case "oAuthUsername":
				return null;
			case "enterprise":
				return account.info.host;
			case "patUsername":
				return "PAT";
		}
	}

	export function tooltipText(account: ButGitHubToken.GithubAccountIdentifier): string {
		switch (account.type) {
			case "oAuthUsername":
				return "";
			case "enterprise":
				return "GitHub Enterprise";
			case "patUsername":
				return "Personal Access Token";
		}
	}
</script>

{#if account.type !== "oAuthUsername"}
	<Badge class={className} tooltip={tooltipText(account)}>
		{badgeText(account)}
	</Badge>
{/if}
