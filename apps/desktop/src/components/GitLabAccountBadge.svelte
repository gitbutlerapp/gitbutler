<script lang="ts">
	import { Badge } from "@gitbutler/ui";
	import type { ButGitLabToken } from "@gitbutler/core/api";

	type Props = {
		account: ButGitLabToken.GitlabAccountIdentifier;
		class?: string;
	};

	const { account, class: className }: Props = $props();

	export function badgeText(account: ButGitLabToken.GitlabAccountIdentifier): string | null {
		switch (account.type) {
			case "patUsername":
				return "PAT";
			case "selfHosted":
				return account.info.host;
		}
	}

	export function tooltipText(account: ButGitLabToken.GitlabAccountIdentifier): string {
		switch (account.type) {
			case "patUsername":
				return "Personal Access Token";
			case "selfHosted":
				return "Self-Hosted GitLab";
		}
	}
</script>

<Badge class={className} tooltip={tooltipText(account)}>
	{badgeText(account)}
</Badge>
