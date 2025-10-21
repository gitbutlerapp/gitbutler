<script lang="ts">
	import {
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		KebabButton,
		TestId
	} from '@gitbutler/ui';

	type Props = {
		onCherryPick: () => void;
		rightClickTrigger: HTMLElement;
	};

	const { onCherryPick: onclick, rightClickTrigger }: Props = $props();

	let kebabButton = $state<HTMLElement>();
	let contextMenu = $state<ContextMenu>();
</script>

<KebabButton
	flat
	bind:el={kebabButton}
	contextElement={rightClickTrigger}
	testId={TestId.KebabMenuButton}
	onclick={() => {
		contextMenu?.open();
	}}
/>

<ContextMenu
	bind:this={contextMenu}
	leftClickTrigger={kebabButton}
	{rightClickTrigger}
	testId={TestId.CommitRowContextMenu}
>
	<ContextMenuSection>
		<ContextMenuItem
			label="Cherry-pick commit"
			icon="cherry-pick"
			onclick={() => {
				contextMenu?.close();
				onclick();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>
