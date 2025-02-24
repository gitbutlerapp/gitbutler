<script lang="ts">
	import AttachmentList from '$components/codegen/AttachmentList.svelte';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		addedDirs: string[];
		onRemoveDir: (dir: string) => void;
	};

	const { addedDirs, onRemoveDir }: Props = $props();
</script>

{#if addedDirs.length > 0}
	<div class="added-dirs-container">
		<div class="flex gap-16">
			<div class="flex items-center gap-8 flex-1">
				<Icon name="folder" color="var(--clr-text-3)" />
				<span class="text-12 clr-text-2">Added Directories</span>
			</div>

			{#if addedDirs.length > 1}
				<button
					type="button"
					class="text-11 text-semibold clear-btn"
					onclick={() => {
						addedDirs.forEach((dir) => onRemoveDir(dir));
					}}
				>
					Clear All
				</button>
			{/if}
		</div>

		<AttachmentList
			attachments={addedDirs.map((dir) => ({
				type: 'directory',
				path: dir
			}))}
			onRemove={(attachment) => {
				if (attachment.type === 'directory') {
					onRemoveDir(attachment.path);
				}
			}}
		/>
	</div>
{/if}

<style lang="postcss">
	.added-dirs-container {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 10px;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.clear-btn {
		color: var(--clr-text-3);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
