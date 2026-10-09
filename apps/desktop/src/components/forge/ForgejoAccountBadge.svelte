<script lang="ts">
	// forgejo-fork
	import { forgejoHostLabel } from "$lib/forge/forgejo/forgejoUserService.svelte";
	import { Badge } from "@gitbutler/ui";
	import type { ForgejoAccountIdentifier } from "@gitbutler/but-sdk";

	type Props = {
		account: ForgejoAccountIdentifier;
		class?: string;
	};

	const { account, class: className }: Props = $props();
</script>

<Badge class={className} tooltip="Forgejo instance {account.host}">
	{forgejoHostLabel(account)}
</Badge>
