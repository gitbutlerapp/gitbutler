<script lang="ts">
	import RulesModal from "$lib/components/rules/RulesModal.svelte";
	import { ContextMenu, ContextMenuItem, ContextMenuSection } from "@gitbutler/ui";
	import { copyToClipboard } from "@gitbutler/ui/utils/clipboard";

	import type { ChatMessage } from "@gitbutler/shared/chat/types";

	type Props = {
		open: boolean;
		leftClickTrigger: HTMLElement | undefined;
		projectSlug: string;
		message: ChatMessage;
		onclose?: () => void;
		onopen?: () => void;
	};

	let { open, leftClickTrigger, message, projectSlug, onclose, onopen }: Props = $props();

	let rulesModal = $state<RulesModal>();

	function copyLink() {
		const url = new URL(window.location.href);
		url.searchParams.set("m", message.uuid);
		copyToClipboard(url.toString());
		onclose?.();
	}

	function openRulesModal() {
		rulesModal?.show();
		onclose?.();
	}
</script>

{#if open}
	<ContextMenu {leftClickTrigger} target={leftClickTrigger} {onclose} {onopen}>
		<ContextMenuSection>
			<ContextMenuItem label="Copy link" onclick={copyLink} />
		</ContextMenuSection>
		<ContextMenuSection>
			<ContextMenuItem label="Create a rule" onclick={openRulesModal} />
		</ContextMenuSection>
	</ContextMenu>
{/if}

<RulesModal {message} {projectSlug} bind:this={rulesModal} />
