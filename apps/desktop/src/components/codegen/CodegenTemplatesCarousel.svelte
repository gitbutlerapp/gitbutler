<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	interface Props {
		templates: Array<{
			label: string;
			emoji?: string;
			fileName?: string;
		}>;
		onInsertTemplate: (template: { label: string; emoji?: string; fileName?: string }) => void;
		onEdit: () => void;
	}

	const { templates, onInsertTemplate, onEdit }: Props = $props();
</script>

<div class="templates-container">
	<div class="templates hide-native-scrollbar">
		{#each templates as template}
			<button type="button" class="templates-card" onclick={() => onInsertTemplate(template)}>
				{#if template.emoji}
					<div class="text-13">{template.emoji}</div>
				{:else}
					<Icon name="script" color="var(--clr-text-2)" />
				{/if}
				<div class="text-13 text-semibold">{template.label}</div>
			</button>
		{/each}

		<button
			type="button"
			class="templates-card templates-edit"
			onclick={onEdit}
			title="Edit Templates"
		>
			<Icon name="mixer" color="var(--clr-text-2)" />
			<div class="text-13 text-semibold">Edit…</div>
		</button>
	</div>

	<p class="templates-caption text-12 text-body">
		Quick-start prompts for common coding tasks.
		<br />
		Select a template to begin, then customize for your needs.
	</p>
</div>

<style lang="postcss">
	.templates-container {
		display: flex;
		flex-direction: column;
		padding-bottom: 16px;
		gap: 8px;
	}

	.templates {
		display: flex;
		position: relative;
		padding: 20px 16px 0;
		overflow-x: auto;
		gap: 8px;

		&::after {
			position: absolute;
			top: 0;
			right: 16px;
			left: 16px;
			height: 1px;
			background-image: repeating-linear-gradient(
				to right,
				var(--clr-border-2) 0,
				var(--clr-border-2) 2px,
				transparent 2px,
				transparent 4px
			);
			content: '';
		}
	}

	.templates-card {
		display: flex;
		flex-grow: 1;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: space-between;
		width: auto;
		min-width: 80px;
		padding: 12px 12px 10px 10px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		text-align: left;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.templates-edit {
		max-width: 80px;
		/* border: none;
		background-color: var(--clr-bg-2);

		&:hover {
			background-color: var(--clr-bg-2-muted);
		} */
	}

	.templates-caption {
		position: relative;
		padding: 8px 16px 0;
		color: var(--clr-text-3);
	}
</style>
