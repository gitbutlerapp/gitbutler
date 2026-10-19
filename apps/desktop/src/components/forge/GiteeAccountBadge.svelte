<script lang="ts">
	import { Badge } from "@gitbutler/ui-svelte";
	import type { GiteeAccountIdentifier } from "@gitbutler/but-sdk";

	type Props = {
		account: GiteeAccountIdentifier;
		class?: string;
	};

	const { account, class: className }: Props = $props();

	export function badgeText(account: GiteeAccountIdentifier): string | null {
		switch (account.type) {
			case "patUsername":
				return "PAT";
			case "selfHosted":
				return account.info.host;
		}
	}

	export function tooltipText(account: GiteeAccountIdentifier): string {
		switch (account.type) {
			case "patUsername":
				return "Personal Access Token";
			case "selfHosted":
				return "Self-Hosted Gitee";
		}
	}
</script>

<Badge class={className} tooltip={tooltipText(account)}>
	{badgeText(account)}
</Badge>
