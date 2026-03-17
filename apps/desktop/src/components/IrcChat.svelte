<script lang="ts">
	import IrcChannel from "$components/IrcChannel.svelte";
	import IrcChannels from "$components/IrcChannels.svelte";
	import Resizer from "$components/Resizer.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";

	import type { Snippet } from "svelte";

	let {
		projectId,
		headerActions,
	}: {
		projectId: string;
		headerActions?: Snippet;
	} = $props();

	const uiState = inject(UI_STATE);
	const currentName = $derived(uiState.global.channel.current);

	let sidebarEl = $state<HTMLDivElement>();
</script>

<div class="irc">
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
			<IrcChannel {projectId} type="server" {headerActions} />
		{:else if currentName.startsWith("#")}
			<IrcChannel {projectId} type="group" channel={currentName} autojoin {headerActions} />
		{:else}
			<IrcChannel {projectId} type="private" nick={currentName} {headerActions} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.irc {
		display: flex;
		width: 100%;
		height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-1);
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
