<script lang="ts">
	import { openExternalUrl } from '$lib/utils/url';
	import LinkButton from '@gitbutler/ui/LinkButton.svelte';
</script>

<div class="error-series">
	<div class="commit-line"></div>
	<div class="text-13 text-body error-series__label">
		This branch failed to load.
		<br />
		<br />
		Please check out our
		<LinkButton
			icon="copy-small"
			onclick={async () => {
				openExternalUrl('http://docs.gitbutler.com/development/debugging');
			}}
		>
			documentation
		</LinkButton> or reload to try again.
	</div>
</div>

<style>
	.error-series {
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background: var(--clr-bg-1);
		scroll-margin-top: 120px;

		&:last-child {
			margin-bottom: 12px;
		}
		display: flex;
		overflow: hidden;
	}
	.error-series__label {
		color: var(--clr-text-2);

		width: 100%;
		padding: 20px 28px 26px 46px;
		opacity: 0.6;
	}
	.commit-line {
		--commit-color: var(--clr-theme-err-element);
		position: absolute;
		left: 20px;
	}
</style>
