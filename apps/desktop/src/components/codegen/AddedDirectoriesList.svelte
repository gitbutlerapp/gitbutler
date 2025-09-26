<script lang="ts">
	import Drawer from '$components/Drawer.svelte';
	import { chipToasts, Icon, Badge, Button } from '@gitbutler/ui';
	import { copyToClipboard } from '@gitbutler/ui/utils/clipboard';

	type Props = {
		addedDirs: string[];
		onRemoveDir: (dir: string) => void;
	};

	const { addedDirs, onRemoveDir }: Props = $props();
</script>

{#if addedDirs.length > 0}
	<Drawer defaultCollapsed={false} noshrink>
		{#snippet header()}
			<h4 class="text-14 text-semibold truncate">Added directories</h4>
			<Badge>{addedDirs.length}</Badge>
		{/snippet}

		<div class="dirs-list">
			{#each addedDirs as dir}
				<div class="added-dir-item">
					<Icon name="folder" color="var(--clr-text-3)" />
					<span class="text-13 flex-1 truncate">{dir}</span>

					<div class="dirs-list__actions">
						<Button
							kind="ghost"
							icon="copy"
							size="tag"
							tooltip="Copy path"
							onclick={() => {
								copyToClipboard(dir);
							}}
						/>

						<Button
							kind="ghost"
							icon="remove-from-list"
							size="tag"
							style="error"
							shrinkable
							onclick={() => {
								onRemoveDir(dir);
								chipToasts.success(`Removed directory: ${dir}`);
							}}
							tooltip="Remove"
						/>
					</div>
				</div>
			{/each}
		</div>
	</Drawer>
{/if}

<style lang="postcss">
	.dirs-list {
		display: flex;
		flex-direction: column;
		padding-bottom: 14px;
	}

	.added-dir-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 36px;
		padding-right: 8px;
		padding-left: 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-3);

		&:hover .dirs-list__actions {
			display: flex;
		}

		&:last-child {
			border-bottom: none;
		}
	}

	.dirs-list__actions {
		display: none;
		align-items: center;
		overflow: hidden;
		gap: 4px;
	}
</style>
