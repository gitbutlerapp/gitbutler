<script lang="ts">
	import { Codeblock } from '@gitbutler/ui';
	import { Modal, Segment, SegmentControl, Button } from '@gitbutler/ui';
	import type { PromptDir } from '$lib/codegen/types';

	type Props = {
		promptDirs: PromptDir[];
		openPromptConfigDir: (path: string) => void;
	};

	let modal = $state<Modal>();

	export function show() {
		modal?.show();
	}

	const { promptDirs, openPromptConfigDir }: Props = $props();

	let selectedSegment = $state<string>(promptDirs[0]?.label || '');
</script>

{#snippet pathContent({ path, caption }: { path: string; caption: string })}
	<Codeblock content={path} label="Location:" />
	<p class="text-13 text-body clr-text-2">{caption}</p>
{/snippet}

<Modal bind:this={modal} width={420} title="Configure prompt templates">
	<div class="stack-v gap-16">
		<p class="text-13 text-body clr-text-2">
			Prompts are searched in two locations. Project prompts override global prompts. Files ending
			in <code class="code-string">.local.md</code> override regular project prompts.
		</p>

		<SegmentControl defaultIndex={0}>
			{#each promptDirs as dir}
				<Segment
					onselect={() => (selectedSegment = dir.label)}
					id={dir.label}
					icon={dir.label === 'Global' ? 'global-small' : 'folder'}
				>
					{dir.label}
				</Segment>
			{/each}
		</SegmentControl>

		{#if selectedSegment === 'Global'}
			{@render pathContent({
				path: promptDirs.find((d) => d.label === 'Global')?.path || '',
				caption: 'Contains global prompt templates available to all projects.'
			})}
		{:else if selectedSegment === 'Project'}
			{@render pathContent({
				path: promptDirs.find((d) => d.label === 'Project')?.path || '',
				caption: 'Contains project-specific prompt templates that override global prompts.'
			})}
		{/if}

		<Button
			icon="open-editor-small"
			onclick={() => {
				const dir = promptDirs.find((d) => d.label === selectedSegment);
				if (dir) {
					openPromptConfigDir(dir.path);
				}
			}}>Open in editor</Button
		>
	</div>
</Modal>

<style lang="postcss">
	.prompt-path {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 10px;
		gap: 4px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		font-family: var(--font-mono);
	}

	.prompt-path__copy {
		position: absolute;
		top: 12px;
		right: 12px;
		color: var(--clr-text-3);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}

	.prompt-path__label {
		color: var(--clr-text-2);
		font-size: 12px;
	}

	.prompt-path__path {
		color: var(--clr-text-1);
		font-size: 13px;
	}
</style>
