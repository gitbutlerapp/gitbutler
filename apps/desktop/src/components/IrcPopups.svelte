<script lang="ts">
	import IrcChat from "$components/IrcChat.svelte";
	import FloatingModal from "$lib/floating/FloatingModal.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button } from "@gitbutler/ui";
	import type { SnapPositionName } from "$lib/floating/types";

	const { projectId }: { projectId: string } = $props();
	const uiState = inject(UI_STATE);

	const ircChatOpen = uiState.global.ircChatOpen;
	const { width, height } = $derived(uiState.global.ircChatSize.current);
	const snapPosition = $derived(uiState.global.ircChatPosition.current);

	let ircChatHeaderEl: HTMLDivElement | undefined = $state();
</script>

{#if ircChatOpen.current}
	<FloatingModal
		defaults={{
			width,
			minWidth: 360,
			height,
			minHeight: 300,
			snapPosition,
		}}
		onUpdateSize={(newWidth, newHeight) => {
			uiState.global.ircChatSize.set({ width: newWidth, height: newHeight });
		}}
		onUpdateSnapPosition={(pos: SnapPositionName) => {
			uiState.global.ircChatPosition.set(pos);
		}}
		dragHandleElement={ircChatHeaderEl}
		onCancel={() => ircChatOpen.set(false)}
	>
		<div class="irc-floating-header" bind:this={ircChatHeaderEl}>
			<span class="text-14 text-semibold">ButNet</span>
			<Button size="tag" icon="cross" kind="ghost" onclick={() => ircChatOpen.set(false)} />
		</div>
		<div class="irc-floating-content">
			<IrcChat {projectId} noBorder />
		</div>
	</FloatingModal>
{/if}

<style lang="postcss">
	.irc-floating-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 8px 8px 12px;
		border-bottom: 1px solid var(--clr-border-3);
		background: var(--clr-bg-2);
		cursor: grab;
	}

	.irc-floating-content {
		display: flex;
		flex-grow: 1;
		overflow: hidden;
	}
</style>
