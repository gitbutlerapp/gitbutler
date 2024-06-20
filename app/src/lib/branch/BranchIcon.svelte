<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';

	export let name: 'remote-branch' | 'virtual-branch' | 'pr' | 'pr-draft' | 'pr-closed' | undefined;
	export let help: string | undefined;

	function getIconColor(name: string | undefined) {
		if (name === 'remote-branch') return 'neutral';
		if (name === 'virtual-branch') return 'virtual';
		if (name === 'pr') return 'success';
		if (name === 'pr-draft') return 'purple';
		if (name === 'pr-closed') return 'neutral';

		return 'neutral';
	}
</script>

<div class="branch-icon {getIconColor(name)}" use:tooltip={help}>
	{#if name === 'virtual-branch'}
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path d="M5.75 8V4H4.25V8H5.75Z" />
			<path
				d="M11.75 7V4H10.25V7C10.25 8.24264 9.24264 9.25 8 9.25C5.92893 9.25 4.25 10.9289 4.25 13V16H5.75V13C5.75 11.7574 6.75736 10.75 8 10.75C10.0711 10.75 11.75 9.07107 11.75 7Z"
			/>
		</svg>
	{/if}
	{#if name === 'remote-branch'}
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M5.75 9.99973V4H4.25V16H5.75V13C5.75 11.7574 6.75736 10.75 8 10.75C10.0711 10.75 11.75 9.07107 11.75 7V4H10.25V7C10.25 8.24264 9.24264 9.25 8 9.25C7.1558 9.25 6.37675 9.52896 5.75 9.99973Z"
			/>
		</svg>
	{/if}
	{#if name === 'pr'}
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M3 5L8 2V4.25H10C11.5188 4.25 12.75 5.48122 12.75 7V16H11.25V7C11.25 6.30964 10.6904 5.75 10 5.75H8V8L3 5Z"
			/>
			<path d="M4.25 9L4.25 16H5.75L5.75 9H4.25Z" />
		</svg>
	{/if}
	{#if name === 'pr-draft'}
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M11.75 7.88555C12.7643 7.56698 13.5 6.61941 13.5 5.5C13.5 4.11929 12.3807 3 11 3C9.61929 3 8.5 4.11929 8.5 5.5C8.5 6.61941 9.23572 7.56698 10.25 7.88555V9H11.75V7.88555Z"
			/>
			<path d="M4.25 16V4L5.75 4L5.75 16H4.25Z" />
			<path d="M10.25 13L10.25 11H11.75L11.75 13H10.25Z" />
			<path d="M10.25 16V15H11.75V16H10.25Z" />
		</svg>
	{/if}
	{#if name === 'pr-closed'}
		<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M9.99999 4.93934L12.4697 2.46967L13.5303 3.53033L11.0607 6L13.5303 8.46967L12.4697 9.53033L9.99999 7.06066L7.53033 9.53033L6.46967 8.46967L8.93933 6L6.46967 3.53033L7.53033 2.46967L9.99999 4.93934Z"
			/>
			<path d="M3.25 4.00001V16H4.75L4.75 4.00001H3.25Z" />
			<path d="M9.25 10L9.25 16H10.75L10.75 10H9.25Z" />
		</svg>
	{/if}
</div>

<style lang="postcss">
	.branch-icon {
		flex-shrink: 0;
		width: 16px;
		height: 16px;
		border-radius: var(--radius-s);
		& svg {
			width: 16px;
			height: 16px;
		}
		& path {
			fill: var(--clr-bg-1);
		}
	}

	.virtual {
		background-color: var(--clr-scale-ntrl-60);
	}

	.neutral {
		background-color: var(--clr-scale-ntrl-50);
	}

	.success {
		background-color: var(--clr-scale-succ-50);
	}

	.purple {
		background-color: var(--clr-scale-purp-50);
	}
</style>
