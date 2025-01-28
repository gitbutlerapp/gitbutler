<script lang="ts">
	import { platformName } from '$lib/platform/platform';
	import { ShortcutService } from '$lib/shortcuts/shortcutService.svelte';
	import { shortcuts } from '$lib/utils/hotkeys';
	import { getContext } from '@gitbutler/shared/context';
	import Modal from '@gitbutler/ui/Modal.svelte';

	let modal: ReturnType<typeof Modal> | undefined = $state();

	const shortcutService = getContext(ShortcutService);
	shortcutService.on('keyboard-shortcuts', () => {
		show();
	});

	export function show() {
		modal?.show();
	}

	function keysStringToArr(keys: string): string[] {
		return keys.split('+').map((key) => {
			if (key === 'Shift') return '⇧';
			if (key === '$mod') {
				if (platformName === 'macos') {
					return '⌘';
				} else {
					return 'Ctrl';
				}
			}
			return key;
		});
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
		column-count: 2;
		column-gap: 20px;
		column-fill: balance;
		margin-top: 4px;
	}

	.shortcuts__group {
		display: flex;
		flex-direction: column;
		gap: 8px;
		page-break-inside: avoid;
		margin-bottom: 16px;

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
		gap: 8px;
		padding: 8px 0 10px;
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
		font-size: 12px;
		min-width: 18px;
		height: 18px;
		padding: 0 4px;
		background-color: var(--clr-bg-2);
		border-radius: var(--radius-m);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.shortcut-description {
		color: var(--clr-text-2);
	}
</style>
