<script lang="ts">
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService.svelte';
	import { shortcuts } from '$lib/utils/hotkeys';
	import { inject } from '@gitbutler/shared/context';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { keysStringToArr } from '@gitbutler/ui/utils/hotkeys';

	let modal: ReturnType<typeof Modal> | undefined = $state();

	const shortcutService = inject(SHORTCUT_SERVICE);
	shortcutService.on('keyboard-shortcuts', () => {
		show();
	});

	export function show() {
		modal?.show();
	}
</script>

{#snippet keysGroup({ title, keys }: { title: string; keys: keyof typeof shortcuts })}
	<div class="shortcuts__group">
		<h2 class="shortcuts__group-title text-13 text-semibold">{title}</h2>
		<ul>
			{#each Object.entries(shortcuts[keys]) as [_, value]}
				<li class="shortcut__item">
					<div class="shortcut__header">
						<h4 class="text-12">{value.title}</h4>

						<div class="shortcut__keys-group">
							{#each keysStringToArr(value.keys) as key}
								<span class="shortcut__key text-12 text-semibold">{key}</span>
							{/each}
						</div>
					</div>

					{#if value.description}
						<span class="text-11 text-body shortcut-description">{value.description}</span>
					{/if}
				</li>
			{/each}
		</ul>
	</div>
{/snippet}

<Modal bind:this={modal} title="Keyboard shortcuts" closeButton>
	<div class="shortcuts">
		{@render keysGroup({ title: 'Global', keys: 'global' })}
		{@render keysGroup({ title: 'Project', keys: 'project' })}
		{@render keysGroup({ title: 'View', keys: 'view' })}
	</div></Modal
>

<style lang="postcss">
	.shortcuts {
		column-gap: 20px;
		margin-top: 4px;
		column-fill: balance;
		column-count: 2;
	}

	.shortcuts__group {
		display: flex;
		flex-direction: column;
		margin-bottom: 16px;
		gap: 8px;
		page-break-inside: avoid;

		&:last-child {
			margin-bottom: 0;
		}
	}

	.shortcuts__group-title {
		color: var(--clr-text-3);
	}

	.shortcut__item {
		display: flex;
		flex-direction: column;
		padding: 8px 0 10px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-3);

		&:last-child {
			border-bottom: none;
		}
	}

	.shortcut__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.shortcut__keys-group {
		display: flex;
		gap: 2px;
	}

	.shortcut__key {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 18px;
		height: 18px;
		padding: 0 4px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		font-size: 12px;
	}

	.shortcut-description {
		color: var(--clr-text-2);
	}
</style>
