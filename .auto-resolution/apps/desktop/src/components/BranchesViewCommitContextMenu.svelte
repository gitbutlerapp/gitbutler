<script lang="ts">
	import { ContextMenuItem, ContextMenuSection, KebabButton, TestId } from '@gitbutler/ui';

	type Props = {
		onCherryPick: () => void;
		rightClickTrigger: HTMLElement;
	};

	const { onCherryPick: onclick, rightClickTrigger }: Props = $props();
</script>

<KebabButton showOnHover contextElement={rightClickTrigger} testId={TestId.KebabMenuButton}>
	{#snippet contextMenu({ close })}
		<ContextMenuSection>
			<ContextMenuItem
				label="Cherry-pick commit"
				icon="cherry-pick"
				onclick={() => {
					close();
					onclick();
				}}
			/>
		</ContextMenuSection>
	{/snippet}
</KebabButton>
