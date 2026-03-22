<script lang="ts">
	import { Badge } from "@gitbutler/ui";
	import type { ButGiteaToken } from "@gitbutler/core/api";

	type Props = {
		account: ButGiteaToken.GiteaAccountIdentifier;
		class?: string;
	};

	const { account, class: className }: Props = $props();

	function normalizedHost(host: string): string {
		return host
			.replace(/^https?:\/\//, "")
			.replace(/\/api\/v1\/?$/, "")
			.replace(/\/$/, "");
	}

	export function badgeText(account: ButGiteaToken.GiteaAccountIdentifier): string {
		return normalizedHost(account.host);
	}

	export function tooltipText(account: ButGiteaToken.GiteaAccountIdentifier): string {
		return `Gitea instance: ${normalizedHost(account.host)}`;
	}
</script>

<Badge class={className} tooltip={tooltipText(account)}>
	{badgeText(account)}
</Badge>
