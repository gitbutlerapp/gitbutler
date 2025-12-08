<script lang="ts">
	import { Codeblock } from '@gitbutler/ui';
	import { Modal, SegmentControl, Button } from '@gitbutler/ui';
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

		<SegmentControl selected={selectedSegment}>
			{#each promptDirs as dir}
				<SegmentControl.Item
					onselect={() => (selectedSegment = dir.label)}
					id={dir.label}
					icon={dir.label === 'Global' ? 'global-small' : 'folder'}
				>
					{dir.label}
				</SegmentControl.Item>
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
