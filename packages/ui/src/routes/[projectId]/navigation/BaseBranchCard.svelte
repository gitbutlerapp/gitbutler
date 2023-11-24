<script lang="ts">
	import { page } from '$app/stores';
	import type { Project } from '$lib/backend/projects';
	import Badge from '$lib/components/Badge.svelte';
	import Button from '$lib/components/Button.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';

	export let project: Project;
	export let branchController: BranchController;
	export let baseBranchService: BaseBranchService;

	$: base$ = baseBranchService.base$;
	$: selected = $page.url.href.endsWith('/base');

	let baseContents: HTMLElement;
	let loading = false;
</script>

<a
	href="/{project.id}/base"
	class="card"
	style:background-color={selected ? 'var(--clr-theme-container-pale)' : undefined}
	bind:this={baseContents}
>
	<div class="icon">
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
			<rect width="16" height="16" rx="4" fill="#FB7D61" />
			<path d="M8 4L12 8L8 12L4 8L8 4Z" fill="white" />
		</svg>
	</div>

	<div class="content">
		<div class="row_1 text-base-13 font-bold">
			<span>Trunk</span>
			{#if ($base$?.behind || 0) > 0}
				<Tooltip label="Unmerged upstream commits">
					<Badge count={$base$?.behind || 0} />
				</Tooltip>
				<Tooltip label="Merge upstream commits into common base">
					<Button
						height="small"
						color="purple"
						{loading}
						on:click={async (e) => {
							e.preventDefault();
							e.stopPropagation();
							loading = true;
							try {
								await branchController.updateBaseBranch();
							} finally {
								loading = false;
							}
						}}
					>
						merge
					</Button>
				</Tooltip>
			{/if}
		</div>
		<div class="row_2 text-base-12">
			{#if $base$?.remoteUrl.includes('github.com')}
				<IconGithub class="h-3 w-3" />
			{:else}
				<Icon name="branch-16" />
			{/if}
			{$base$?.branchName}
		</div>
	</div>
</a>

<style lang="postcss">
	.card {
		display: flex;
		gap: var(--space-10);
		padding: var(--space-8);
		border-radius: var(--radius-m);
		&:hover,
		&:focus {
			background-color: var(--clr-theme-container-pale);
		}
	}
	.icon {
		flex-shrink: 0;
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
	}
	.row_1 {
		display: flex;
		gap: var(--space-6);
		align-items: center;
		color: var(--clr-theme-scale-ntrl-10);
	}
	.row_2 {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		color: var(--clr-theme-scale-ntrl-40);
	}
</style>
