<script lang="ts">
	import { Button, Modal, SectionCard } from '@gitbutler/ui';
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
</script>

<Modal bind:this={modal} title="Configure prompt templates">
	<div class="flex flex-col gap-16">
		<p class="text-13">
			We have a tierd prompt configuration setup. Prompts are expected to be found in the following
			locations.
		</p>
		<p class="text-13">
			Project prompts take precidence over global prompts, and local prompts take precidence over
			project prompts.
		</p>

		<div>
			{#each promptDirs as dir, idx}
				<SectionCard
					roundedTop={idx === 0}
					roundedBottom={idx === promptDirs.length - 1}
					orientation="row"
				>
					{#snippet title()}
						{dir.label}
					{/snippet}

					{#snippet caption()}
						<div class="flex flex-col gap-6">
							<p class="text-11">{dir.path}{dir.path.endsWith('/') ? '' : '/'}</p>
							<p class="text-13">
								Looks for files ending with: <span class="clr-text-1"
									>{dir.filters.join(' or ')}</span
								>
							</p>
						</div>
					{/snippet}

					{#snippet actions()}
						<Button onclick={() => openPromptConfigDir(dir.path)}>Open in Editor</Button>
					{/snippet}
				</SectionCard>
			{/each}
		</div>
	</div>
</Modal>
