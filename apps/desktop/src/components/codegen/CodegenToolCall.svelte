<script lang="ts">
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { getToolIcon } from '$lib/utils/codegenTools';
	import { DropdownButton, ContextMenuItem, Icon, Markdown } from '@gitbutler/ui';
	import type { PermissionDecision } from '$lib/codegen/types';

	export type RequiresApproval = {
		onPermissionDecision: (id: string, decision: PermissionDecision) => Promise<void>;
	};

	type Props = {
		style?: 'nested' | 'standalone';
		toolCall: ToolCall;
		requiresApproval?: RequiresApproval;
		fullWidth?: boolean;
	};
	const { toolCall, style, requiresApproval = undefined, fullWidth }: Props = $props();

	let expanded = $derived(!!requiresApproval);
	let allowDropdownButton = $state<ReturnType<typeof DropdownButton>>();
	let denyDropdownButton = $state<ReturnType<typeof DropdownButton>>();

	// Persisted state for selected permission scopes
	type AllowDecision = 'allowOnce' | 'allowSession' | 'allowProject' | 'allowAlways';
	type DenyDecision = 'denyOnce' | 'denySession' | 'denyProject' | 'denyAlways';

	let selectedAllowDecision = $state<AllowDecision>('allowSession');
	let selectedDenyDecision = $state<DenyDecision>('denySession');

	// Labels for each decision type
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
</script>

<div class="tool-call {style}" class:full-width={fullWidth}>
	<button
		type="button"
		class="tool-call-header"
		class:expanded
		onclick={() => (expanded = !expanded)}
	>
		<div class="tool-call-header__sublevel"></div>

		<div class="tool-call-header__arrow">
			<Icon name="chevron-right" />
		</div>

		{#if toolCallLoading(toolCall)}
			<Icon name="spinner" />
		{:else}
			<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
		{/if}

		<p class="text-13 text-left full-width truncate">{toolCall.name}</p>

		{#if requiresApproval}
			<div class="flex gap-4 m-l-8">
				<DropdownButton
					bind:this={denyDropdownButton}
					style="error"
					kind="outline"
					onclick={async () => {
						await requiresApproval.onPermissionDecision(toolCall.id, selectedDenyDecision);
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
						await requiresApproval.onPermissionDecision(toolCall.id, selectedAllowDecision);
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
	</button>

	{#if expanded}
		<div class="tool-call-content">
			<Markdown content={`\`\`\`\nRequest:\n${JSON.stringify(toolCall.input)}\n\`\`\``} />
			{#if toolCall.result}
				<Markdown
					content={`\`\`\`\nResponse:\n${toolCall.result.replaceAll('```', '\\`\\`\\`')}\n\`\`\``}
				/>
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.tool-call {
		display: flex;
		flex-direction: column;

		max-width: 100%;
		overflow: hidden;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}

		&:not(.full-width) {
			max-width: fit-content;
		}

		&.full-width {
			width: 100%;
		}
	}

	.tool-call-header {
		display: flex;
		position: relative;
		align-items: center;
		padding: 10px 12px 10px 22px;
		gap: 8px;
		background-color: var(--clr-bg-2);

		&:hover {
			background-color: var(--clr-bg-2-muted);

			.tool-call-header__arrow {
				color: var(--clr-text-2);
			}
		}

		&.expanded {
			border-bottom: 1px solid var(--clr-border-3);

			.tool-call-header__arrow {
				transform: rotate(90deg);
			}
		}
	}

	.tool-call-header__sublevel {
		position: absolute;
		top: 0;
		left: 15px;
		width: 1px;
		height: 100%;
		background-color: var(--clr-border-2);
	}

	.tool-call-header__arrow {
		display: flex;
		color: var(--clr-text-3);
		transition:
			color var(--transition-fast),
			transform var(--transition-medium);
	}

	.tool-call-content {
		display: flex;
		flex-direction: column;
		max-width: 100%;
		padding: 12px;
		gap: 8px;
	}

	.tool-call-content :global(pre) {
		margin: 0;
		padding-bottom: 0px;
		overflow-x: scroll;
	}

	/* STANDALONE MODE */
	.tool-call.standalone {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);

		.tool-call-header {
			padding-left: 12px;
			border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		}

		.tool-call-header__sublevel {
			display: none;
		}
	}
</style>
