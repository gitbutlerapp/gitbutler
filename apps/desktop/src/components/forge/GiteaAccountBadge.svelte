<script lang="ts">
	import { Badge } from "@gitbutler/ui";

	type Props = {
		account: any;
		class?: string;
	};

	const { account, class: className }: Props = $props();

	export function badgeText(account: any): string | null {
		// For Gitea, we always show the host as it's self-hosted
		return account.info.host;
	}

	export function tooltipText(account: any): string {
		return "Gitea Account";
	}
</script>

<Badge class={className} tooltip={tooltipText(account)}>
	{badgeText(account)}
</Badge>
