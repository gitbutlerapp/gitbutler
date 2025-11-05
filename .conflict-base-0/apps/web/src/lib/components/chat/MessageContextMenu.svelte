<script lang="ts">
	import RulesModal from '$lib/components/rules/RulesModal.svelte';
	import { ContextMenu, ContextMenuItem, ContextMenuSection } from '@gitbutler/ui';
	import { copyToClipboard } from '@gitbutler/ui/utils/clipboard';

	import type { ChatMessage } from '@gitbutler/shared/chat/types';

	type Props = {
		menu: ReturnType<typeof ContextMenu> | undefined;
		leftClickTrigger: HTMLElement | undefined;
		projectSlug: string;
		message: ChatMessage;
		onToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	};

	let { menu = $bindable(), leftClickTrigger, message, projectSlug, onToggle }: Props = $props();

	let rulesModal = $state<RulesModal>();

	function copyLink() {
		const url = new URL(window.location.href);
		url.searchParams.set('m', message.uuid);
		copyToClipboard(url.toString());
		menu?.close();
	}

	function openRulesModal() {
		rulesModal?.show();
		menu?.close();
	}
</script>

<ContextMenu bind:this={menu} {leftClickTrigger} ontoggle={onToggle}>
	<ContextMenuSection>
		<ContextMenuItem label="Copy link" onclick={copyLink} />
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem label="Create a rule" onclick={openRulesModal} />
	</ContextMenuSection>
</ContextMenu>

<RulesModal {message} {projectSlug} bind:this={rulesModal} />
