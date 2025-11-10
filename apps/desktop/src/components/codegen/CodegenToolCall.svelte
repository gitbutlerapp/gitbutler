<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { formatToolCall, getToolIcon } from '$lib/utils/codegenTools';
	import { DropdownButton, ContextMenuItem, Icon, Select, SelectItem } from '@gitbutler/ui';
	import type { PermissionDecision } from '$lib/codegen/types';

	export type RequiresApproval = {
		onPermissionDecision: (
			id: string,
			decision: PermissionDecision,
			useWildcard: boolean
		) => Promise<void>;
	};

	type Props = {
		projectId: string;
		style?: 'nested' | 'standalone';
		toolCall: ToolCall;
		requiresApproval?: RequiresApproval;
		fullWidth?: boolean;
	};
	const { toolCall, style, requiresApproval = undefined, fullWidth }: Props = $props();

	let expanded = $state(false);
	let resultDiv = $state<HTMLDivElement>();
	let resultContentDiv = $state<HTMLDivElement>();
	let allowDropdownButton = $state<ReturnType<typeof DropdownButton>>();
	let denyDropdownButton = $state<ReturnType<typeof DropdownButton>>();

	type AllowDecision = 'allowOnce' | 'allowSession' | 'allowProject' | 'allowAlways';
	type DenyDecision = 'denyOnce' | 'denySession' | 'denyProject' | 'denyAlways';
	type WildcardDecision = 'precise' | 'wild';

	let selectedAllowDecision = $state<AllowDecision>('allowSession');
	let selectedDenyDecision = $state<DenyDecision>('denySession');
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

<div class="tool-call {style}" class:full-width={fullWidth}>
	<button
		type="button"
		class="tool-details text-13"
		class:expanded
		onclick={() => {
			expanded = !expanded;
		}}
	>
		<div class="tool-call-header__arrow">
			<Icon name="chevron-right" />
		</div>
		{#if toolCallLoading(toolCall)}
			<Icon name="spinner" />
		{:else}
			<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
		{/if}

		<span class="tool-name text-12">{toolCall.name}</span>

		<span class="summary truncate grow clr-text-2">{formatToolCall(toolCall)}</span>
	</button>

	{#if requiresApproval}
		<div class="flex gap-4">
			{#if wildcardSelector.show}
				<Select
					value={selectedWildcardDecision}
					options={wildcardSelector.options}
					wide
					onselect={(value) => {
						selectedWildcardDecision = value as WildcardDecision;
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === selectedWildcardDecision} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>
			{/if}

			<DropdownButton
				bind:this={denyDropdownButton}
				style="error"
				kind="outline"
				onclick={async () => {
					await requiresApproval.onPermissionDecision(
						toolCall.id,
						selectedDenyDecision,
						selectedWildcardDecision === 'wild'
					);
					denyDropdownButton?.close();
				}}
			>
				{denyLabels[selectedDenyDecision]}
				{#snippet contextMenuSlot()}
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
				{/snippet}
			</DropdownButton>
			<DropdownButton
				bind:this={allowDropdownButton}
				style="pop"
				onclick={async () => {
					await requiresApproval.onPermissionDecision(
						toolCall.id,
						selectedAllowDecision,
						selectedWildcardDecision === 'wild'
					);
					allowDropdownButton?.close();
				}}
			>
				{allowLabels[selectedAllowDecision]}
				{#snippet contextMenuSlot()}
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
				{/snippet}
			</DropdownButton>
		</div>
	{/if}

	{#if expanded && toolCall.result}
		<div
			bind:this={resultDiv}
			class="tool-call-wrapper text-13"
			class:border={resultDiv &&
				resultContentDiv &&
				resultDiv.clientHeight < resultContentDiv.clientHeight}
		>
			<ConfigurableScrollableContainer maxHeight="20lh">
				<div bind:this={resultContentDiv} class="tool-call-content text-13">
					{toolCall.result.slice(0, 65536)}
				</div>
			</ConfigurableScrollableContainer>
		</div>
	{/if}
</div>

<style lang="postcss">
	.tool-call {
		display: flex;
		flex-direction: column;

		max-width: 100%;

		padding: 1px 32px 1px 12px;
		overflow: hidden;

		gap: 12px;
		user-select: text;

		&:not(.full-width) {
			max-width: fit-content;
		}

		&.full-width {
			width: 100%;
		}
		&.standalone {
			padding: 12px 0;
		}
	}

	.tool-details {
		display: flex;
		position: relative;
		align-items: center;
		padding: 2px 0;
		gap: 8px;

		&:hover {
			.tool-call-header__arrow {
				color: var(--clr-text-2);
			}
		}
	}

	.tool-call-header__arrow {
		display: flex;
		color: var(--clr-text-3);
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);
	}

	.expanded .tool-call-header__arrow {
		transform: rotate(90deg);
	}

	.tool-call-wrapper {
		margin-bottom: 12px;
		overflow: hidden;
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-2);

		&.border {
			border: 1px solid var(--clr-border-3);
		}
	}

	.tool-call-content {
		padding: 10px 14px;
		font-family: var(--font-mono);
		white-space: pre-line;
	}

	.tool-call-content :global(pre) {
	}

	.tool-name {
		white-space: nowrap;
	}

	.summary {
		font-family: var(--font-mono);
	}
</style>
