<script lang="ts">
	import Codeblock from '$components/codegen/Codeblock.svelte';
	import { type ToolCall } from '$lib/codegen/messages';
	import { formatToolCall, getToolIcon } from '$lib/utils/codegenTools';
	import {
		DropdownButton,
		ContextMenuItem,
		ContextMenuSection,
		ContextMenu,
		Button,
		Icon
	} from '@gitbutler/ui';
	import type { PermissionDecision } from '$lib/codegen/types';

	type Props = {
		projectId: string;

		toolCall: ToolCall;
		onPermissionDecision: (
			id: string,
			decision: PermissionDecision,
			useWildcard: boolean
		) => Promise<void>;
	};
	const { toolCall, onPermissionDecision }: Props = $props();

	let allowDropdownButton = $state<ReturnType<typeof DropdownButton>>();
	let denyDropdownButton = $state<ReturnType<typeof DropdownButton>>();
	let wildcardButton = $state<HTMLElement>();
	let wildcardContextMenu = $state<ReturnType<typeof ContextMenu>>();

	type AllowDecision = 'allowOnce' | 'allowSession' | 'allowProject' | 'allowAlways';
	type DenyDecision = 'denyOnce' | 'denySession' | 'denyProject' | 'denyAlways';
	type WildcardDecision = 'precise' | 'wild';

	let selectedAllowDecision = $state<AllowDecision>('allowOnce');
	let selectedDenyDecision = $state<DenyDecision>('denyOnce');
	let selectedWildcardDecision = $state<WildcardDecision>('precise');

	const allowLabels: Record<AllowDecision, string> = {
		allowOnce: 'Allow once',
		allowProject: 'Allow this project',
		allowSession: 'Allow in this session',
		allowAlways: 'Allow always'
	};

	const denyLabels: Record<DenyDecision, string> = {
		denyOnce: 'Deny once',
		denySession: 'Deny this session',
		denyProject: 'Deny this project',
		denyAlways: 'Deny always'
	};

	// The wildcard selector only shows up for certain tool calls
	const wildcardSelector = $derived.by<
		{ show: false } | { show: true; options: { label: string; value: WildcardDecision }[] }
	>(() => {
		if (toolCall.name === 'Edit' || toolCall.name === 'Write') {
			return {
				show: true,
				options: [
					{ value: 'precise', label: 'This file' },
					{ value: 'wild', label: 'Any files in the same folder' }
				]
			};
		} else if (toolCall.name === 'Bash') {
			return {
				show: true,
				options: [
					{ value: 'precise', label: 'This command' },
					{ value: 'wild', label: 'Any subcommands or flags' }
				]
			};
		} else {
			return { show: false };
		}
	});
</script>

<div class="tool-call">
	<div class="tool-call__details">
		<div class="tool-call__header">
			<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
			<span class="text-13 tool-name">{toolCall.name}</span>
		</div>

		<Codeblock content={formatToolCall(toolCall)} />
	</div>

	<div class="tool-call__actions">
		{#if wildcardSelector.show}
			<Button
				bind:el={wildcardButton}
				kind="outline"
				icon="select-chevron"
				shrinkable
				onclick={() => {
					wildcardContextMenu?.toggle();
				}}
			>
				{wildcardSelector.options.find((opt) => opt.value === selectedWildcardDecision)?.label ||
					'Select scope'}
			</Button>

			<ContextMenu bind:this={wildcardContextMenu} leftClickTrigger={wildcardButton}>
				<ContextMenuSection>
					{#each wildcardSelector.options as option}
						<ContextMenuItem
							label={option.label}
							selected={option.value === selectedWildcardDecision}
							onclick={() => {
								selectedWildcardDecision = option.value;
								wildcardContextMenu?.close();
							}}
						/>
					{/each}
				</ContextMenuSection>
			</ContextMenu>
		{/if}

		<DropdownButton
			bind:this={denyDropdownButton}
			style="error"
			kind="outline"
			onclick={async () => {
				await onPermissionDecision(
					toolCall.id,
					selectedDenyDecision,
					selectedWildcardDecision === 'wild'
				);
				denyDropdownButton?.close();
			}}
		>
			{denyLabels[selectedDenyDecision]}
			{#snippet contextMenuSlot()}
				<ContextMenuSection>
					<ContextMenuItem
						label="Deny once"
						onclick={() => {
							selectedDenyDecision = 'denyOnce';
							denyDropdownButton?.close();
						}}
					/>
					<ContextMenuItem
						label="Deny in this session"
						onclick={() => {
							selectedDenyDecision = 'denySession';
							denyDropdownButton?.close();
						}}
					/>
					<ContextMenuItem
						label="Deny in this project"
						onclick={() => {
							selectedDenyDecision = 'denyProject';
							denyDropdownButton?.close();
						}}
					/>
					<ContextMenuItem
						label="Deny always"
						onclick={() => {
							selectedDenyDecision = 'denyAlways';
							denyDropdownButton?.close();
						}}
					/>
				</ContextMenuSection>
			{/snippet}
		</DropdownButton>

		<DropdownButton
			bind:this={allowDropdownButton}
			style="pop"
			onclick={async () => {
				await onPermissionDecision(
					toolCall.id,
					selectedAllowDecision,
					selectedWildcardDecision === 'wild'
				);
				allowDropdownButton?.close();
			}}
		>
			{allowLabels[selectedAllowDecision]}
			{#snippet contextMenuSlot()}
				<ContextMenuSection>
					<ContextMenuItem
						label="Allow once"
						onclick={() => {
							selectedAllowDecision = 'allowOnce';
							allowDropdownButton?.close();
						}}
					/>
					<ContextMenuItem
						label="Allow in this session"
						onclick={() => {
							selectedAllowDecision = 'allowSession';
							allowDropdownButton?.close();
						}}
					/>
					<ContextMenuItem
						label="Allow in this project"
						onclick={() => {
							selectedAllowDecision = 'allowProject';
							allowDropdownButton?.close();
						}}
					/>
					<ContextMenuItem
						label="Allow always"
						onclick={() => {
							selectedAllowDecision = 'allowAlways';
							allowDropdownButton?.close();
						}}
					/>
				</ContextMenuSection>
			{/snippet}
		</DropdownButton>
	</div>
</div>

<style lang="postcss">
	.tool-call {
		display: flex;
		flex-direction: column;
		max-width: 100%;
		max-width: var(--message-max-width);
		margin-bottom: 10px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		user-select: text;
	}

	.tool-call__details {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 10px;
	}

	.tool-call__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.tool-call__actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: 12px;
		gap: 6px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
