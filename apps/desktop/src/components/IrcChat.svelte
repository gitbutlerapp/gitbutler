<script lang="ts">
	import IrcChannel from "$components/IrcChannel.svelte";
	import IrcChannels from "$components/IrcChannels.svelte";
	import Resizer from "$components/Resizer.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";

	const { projectId, noBorder = false }: { projectId: string; noBorder?: boolean } = $props();

	const uiState = inject(UI_STATE);
	const currentName = $derived(uiState.global.channel.current);

	let sidebarEl = $state<HTMLDivElement>();
</script>

<div class="irc" class:no-border={noBorder}>
	<div class="sidebar" bind:this={sidebarEl}>
		<IrcChannels {projectId} />
		{#if sidebarEl}
			<Resizer
				viewport={sidebarEl}
				direction="right"
				defaultValue={10}
				minWidth={8}
				maxWidth={24}
				persistId="irc-sidebar-width"
			/>
		{/if}
	</div>
	<div class="right">
		{#if currentName === "*" || !currentName}
			<IrcChannel {projectId} type="server" />
		{:else if currentName.startsWith("#")}
			<IrcChannel {projectId} type="group" channel={currentName} autojoin />
		{:else}
			<IrcChannel {projectId} type="private" nick={currentName} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.irc {
		display: flex;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}
	.irc.no-border {
		border: none;
		border-radius: 0;
	}
	.sidebar {
		position: relative;
		flex-shrink: 0;
	}
	.right {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}
</style>
