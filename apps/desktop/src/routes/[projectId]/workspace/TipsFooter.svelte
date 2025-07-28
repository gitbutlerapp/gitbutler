<script lang="ts">
	import { Icon, Tooltip } from '@gitbutler/ui';
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
	.tip-footer__links {
		display: flex;
		border-top: 1px solid var(--clr-border-3);
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
