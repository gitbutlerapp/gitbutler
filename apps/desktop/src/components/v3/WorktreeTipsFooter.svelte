<script lang="ts">
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { slide } from 'svelte/transition';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	type gitButlerLinkType = Array<{
		label: string;
		href: string;
		icon: keyof typeof iconsJson;
	}>;

	let buttonLabeles = ['Dependent branches', 'Independent branches', 'Drag & Drop Commits'];
	let gitButlerLinks = [
		{
			label: 'GitButler Docs',
			href: 'https://docs.gitbutler.com',
			icon: 'doc'
		},
		{
			label: 'Source Code',
			href: 'https://github.com/gitbutlerapp/gitbutler',
			icon: 'open-source'
		},
		{ label: 'Join Community', href: 'https://discord.com/invite/MmFkmaJ42D', icon: 'discord' }
	] as gitButlerLinkType;

	const [uiState] = inject(UiState);
	const selectedTip = $derived(uiState.global.selectedTip.get().current);
</script>

{#snippet tipButton(props: { label: string; index: number })}
	{@const { label, index } = props}
	{@const selected = selectedTip === index}
	<button
		type="button"
		class="focus-state text-13 text-semibold text-body tip-button"
		class:selected
		onclick={() => {
			uiState.global.selectedTip.set(index);
		}}
	>
		{#if selected}
			<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
		{/if}
		{label}
	</button>
{/snippet}

{#snippet GbLink(props: { label: string; href: string; icon: keyof typeof iconsJson })}
	{@const { label, href, icon } = props}
	<Tooltip text={label} position="top">
		<a
			type="button"
			{href}
			target="_blank"
			class="focus-state text-13 text-semibold text-body tip-footer__link"
			><Icon name={icon} />
		</a>
	</Tooltip>
{/snippet}

<div
	class="tip-footer"
	role="presentation"
	tabindex="-1"
	onkeydown={(e: KeyboardEvent) => {
		if (e.key === 'Escape') {
			uiState.global.selectedTip.set(undefined);
		}
	}}
>
	<div class="tip-footer__tips">
		<h3 class="text-14 text-semibold tip-footer__group-title">Tips</h3>
		<div class="tip-footer__group-list">
			{#each buttonLabeles as label, index}
				{@render tipButton({ label, index })}
			{/each}
		</div>
	</div>
	<div class="tip-footer__links">
		{#each gitButlerLinks as link}
			{@render GbLink({ label: link.label, href: link.href, icon: link.icon })}
		{/each}
	</div>
</div>

<style lang="postcss">
	.tip-footer {
		display: flex;
		flex-direction: column;

		&:focus {
			outline: none;
		}
	}

	.tip-footer__tips,
	.tip-footer__links {
		display: flex;

		border-top: 1px solid var(--clr-border-3);
	}

	.tip-footer__tips {
		flex-direction: column;
		padding: 18px 16px;
		gap: 12px;
	}

	.tip-footer__group-list {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 10px;
	}

	.tip-button {
		position: relative;
		text-align: left;
		color: var(--clr-text-3);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}

		&.selected {
			color: var(--clr-text-1);
		}
	}

	.active-page-indicator {
		position: absolute;
		left: -16px;
		top: 2px;
		width: 6px;
		height: 18px;
		background-color: var(--clr-text-2);
		border-top-right-radius: var(--radius-s);
		border-bottom-right-radius: var(--radius-s);
	}

	.tip-footer__links {
		gap: 2px;
		align-items: center;
		justify-content: flex-end;
		padding: 12px 14px;
	}

	.tip-footer__link {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 4px 6px;
		color: var(--clr-text-3);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
