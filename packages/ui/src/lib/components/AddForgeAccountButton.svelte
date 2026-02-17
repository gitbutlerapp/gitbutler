<script lang="ts">
	import Button from "$components/Button.svelte";
	import ContextMenu from "$components/ContextMenu.svelte";
	import ContextMenuItem from "$components/ContextMenuItem.svelte";
	import ContextMenuSection from "$components/ContextMenuSection.svelte";
	import type { IconName } from "$components/Icon.svelte";

	interface MenuItem {
		label: string;
		icon: IconName;
		onclick: () => void;
	}

	interface Props {
		noAccounts: boolean;
		disabled?: boolean;
		loading?: boolean;
		menuItems: MenuItem[];
	}

	let { noAccounts, disabled = false, loading = false, menuItems }: Props = $props();

	let addProfileButtonRef = $state<HTMLElement>();
	let addAccountContextMenu = $state<ContextMenu>();

	const buttonText = $derived(noAccounts ? "Add account" : "Add another account");
</script>

<Button
	bind:el={addProfileButtonRef}
	kind="outline"
	onclick={() => addAccountContextMenu?.toggle()}
	{disabled}
	{loading}
	icon="plus-small"
>
	{buttonText}
</Button>

<ContextMenu bind:this={addAccountContextMenu} leftClickTrigger={addProfileButtonRef}>
	<ContextMenuSection>
		{#each menuItems as item}
			<ContextMenuItem
				label={item.label}
				icon={item.icon}
				onclick={() => {
					item.onclick();
					addAccountContextMenu?.close();
				}}
			/>
		{/each}
	</ContextMenuSection>
</ContextMenu>
