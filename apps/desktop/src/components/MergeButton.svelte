<script lang="ts">
	import { MergeMethod } from '$lib/forge/interface/types';
	import { persisted, type Persisted } from '@gitbutler/shared/persisted';

	import { ContextMenuItem, ContextMenuSection, DropdownButton, Checkbox } from '@gitbutler/ui';
	import type { ButtonProps } from '@gitbutler/ui';

	interface Props {
		projectId: string;
		onclick: (method: MergeMethod, bypassRules?: boolean) => Promise<void>;
		disabled?: boolean;
		wide?: boolean;
		tooltip?: string;
		style?: ButtonProps['style'];
		kind?: ButtonProps['kind'];
	}

	const {
		projectId,
		onclick,
		disabled = false,
		wide = false,
		tooltip = '',
		style = 'neutral',
		kind = 'outline'
	}: Props = $props();

	function persistedAction(projectId: string): Persisted<MergeMethod> {
		const key = 'projectMergeMethod';
		return persisted<MergeMethod>(MergeMethod.Merge, key + projectId);
	}

	function persistedBypassRules(projectId: string): Persisted<boolean> {
		const key = 'projectMergeBypassRules';
		return persisted<boolean>(false, key + projectId);
	}

	const action = persistedAction(projectId);
	const bypassRules = persistedBypassRules(projectId);

	let dropDown: ReturnType<typeof DropdownButton> | undefined;
	let loading = $state(false);

	const labels = {
		[MergeMethod.Merge]: 'Merge pull request',
		[MergeMethod.Rebase]: 'Rebase and merge',
		[MergeMethod.Squash]: 'Squash and merge'
	};
</script>

<DropdownButton
	bind:this={dropDown}
	onclick={async () => {
		loading = true;
		try {
			await onclick?.($action, $bypassRules);
		} finally {
			loading = false;
		}
	}}
	{style}
	{kind}
	{loading}
	{wide}
	{tooltip}
	{disabled}
>
	{labels[$action]}
	{#snippet contextMenuSlot()}
		<ContextMenuSection>
			{#each Object.values(MergeMethod) as method}
				<ContextMenuItem
					label={labels[method]}
					onclick={() => {
						$action = method;
						dropDown?.close();
					}}
				/>
			{/each}
		</ContextMenuSection>
		<ContextMenuSection>
			<div class="bypass-checkbox-container">
				<Checkbox
					small
					bind:checked={$bypassRules}
					onchange={() => {
						// The reactive store will handle the change
					}}
				/>
				<span class="text-12 text-light bypass-label">Bypass branch protection rules</span>
			</div>
		</ContextMenuSection>
	{/snippet}
</DropdownButton>

<style lang="postcss">
	.bypass-checkbox-container {
		display: flex;
		align-items: center;
		padding: 8px 12px;
		gap: 8px;
		cursor: pointer;
	}

	.bypass-label {
		cursor: pointer;
		user-select: none;
	}
</style>
