<script lang="ts">
	import Button from "$components/Button.svelte";
	import ContextMenu from "$components/ContextMenu.svelte";
	import ContextMenuItem from "$components/ContextMenuItem.svelte";
	import ContextMenuSection from "$components/ContextMenuSection.svelte";
	import type { IconName } from "$lib/icons/names";

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
	let menuOpen = $state(false);

	const buttonText = $derived(noAccounts ? "Add account" : "Add another account");
</script>

<Button
	bind:el={addProfileButtonRef}
	kind="outline"
	onclick={() => (menuOpen = !menuOpen)}
	{disabled}
	{loading}
	icon="plus"
>
	{buttonText}
</Button>

{#if menuOpen}
	<ContextMenu
		target={addProfileButtonRef}
		leftClickTrigger={addProfileButtonRef}
		onclose={() => (menuOpen = false)}
	>
		<ContextMenuSection>
			{#each menuItems as item}
				<ContextMenuItem
					label={item.label}
					icon={item.icon}
					onclick={() => {
						item.onclick();
						menuOpen = false;
					}}
				/>
			{/each}
		</ContextMenuSection>
	</ContextMenu>
{/if}
