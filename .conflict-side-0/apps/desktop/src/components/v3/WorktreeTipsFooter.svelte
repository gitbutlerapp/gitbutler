<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	import type iconsJson from '@gitbutler/ui/data/icons.json';

	type gitButlerLinkType = Array<{
		label: string;
		href: string;
		icon: keyof typeof iconsJson;
	}>;

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
</script>

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

<div class="tip-footer__links">
	{#each gitButlerLinks as link}
		{@render GbLink({ label: link.label, href: link.href, icon: link.icon })}
	{/each}
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
		color: var(--clr-text-3);
		text-align: left;
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
		top: 2px;
		left: -16px;
		width: 6px;
		height: 18px;
		border-top-right-radius: var(--radius-s);
		border-bottom-right-radius: var(--radius-s);
		background-color: var(--clr-text-2);
	}

	.tip-footer__links {
		align-items: center;
		justify-content: flex-end;
		padding: 12px 14px;
		gap: 2px;
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
