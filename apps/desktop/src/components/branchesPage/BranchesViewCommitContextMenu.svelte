<script lang="ts">
	import { t } from "$lib/i18n";

	import { ContextMenuItem, ContextMenuSection, KebabButton, TestId } from "@gitbutler/ui-svelte";

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
				label={t("cherry-pick-commit")}
				icon="cherry-pick"
				onclick={() => {
					close();
					onclick();
				}}
			/>
		</ContextMenuSection>
	{/snippet}
</KebabButton>
