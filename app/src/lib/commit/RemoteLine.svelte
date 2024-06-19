<script lang="ts">
	import type { CommitStatus } from '$lib/vbranches/types';

	export let root = false;
	export let base = false;
	export let integrated = false;
	export let sectionFirst = false;

	export let inType: CommitStatus | undefined;
	export let outType: CommitStatus | undefined;

	export let inDashed = false;
	export let outDashed = false;
</script>

<div class="remote-column" class:has-root={root} class:base>
	{#if base}
		<div class="remote-line dashed short"></div>
		{#if outType}
			<div
				class="remote-line base tip"
				class:dashed={outDashed}
				class:upstream={outType === 'remote'}
			></div>
		{/if}
		{#if root}
			<div class="root base"></div>
		{/if}
		<div class="base-icon">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="16"
				height="16"
				viewBox="0 0 16 16"
				fill="none"
			>
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M4.32501 7.25C4.67247 5.53832 6.18578 4.25 8 4.25C9.81422 4.25 11.3275 5.53832 11.675 7.25H14V8.75H11.675C11.3275 10.4617 9.81422 11.75 8 11.75C6.18578 11.75 4.67247 10.4617 4.32501 8.75H2V7.25H4.32501ZM8 5.75C6.75736 5.75 5.75 6.75736 5.75 8C5.75 9.24264 6.75736 10.25 8 10.25C9.24264 10.25 10.25 9.24264 10.25 8C10.25 6.75736 9.24264 5.75 8 5.75Z"
					fill="white"
				/>
			</svg>
		</div>
	{:else}
		{#if outType}
			<div
				class="remote-line tip"
				class:integrated
				class:upstream={outType === 'remote'}
				class:remote={outType === 'localAndRemote'}
				class:dashed={outDashed}
				class:first={sectionFirst}
			></div>
		{/if}
		{#if inType}
			<div
				class="remote-line short"
				class:integrated
				class:first={sectionFirst}
				class:upstream={inType === 'remote'}
				class:remote={inType === 'localAndRemote'}
				class:dashed={inDashed}
			></div>
		{/if}
		{#if root}
			<div class="root"></div>
		{/if}
		<slot />
	{/if}
</div>

<style lang="postcss">
	.remote-column {
		position: relative;
		width: 24px;
	}

	.remote-line {
		position: absolute;
		width: 2px;
		background-color: var(--clr-commit-remote);
		left: 11px;
		bottom: 0;
		top: 0;
		&.short {
			top: calc(var(--avatar-top) + 4px);
			&.first {
				top: calc(var(--avatar-first-top) + 4px);
			}
		}
		&.tip {
			bottom: calc(100% - var(--avatar-top) - 4px);
			&.first {
				bottom: calc(100% - var(--avatar-first-top) - 4px);
			}
			&.base {
				bottom: calc(100% - 24px);
			}
		}
		&.dashed {
			background: repeating-linear-gradient(
				0,
				transparent,
				transparent 3px,
				var(--clr-commit-remote) 3px,
				var(--clr-commit-remote) 7px
			);
		}
		&.upstream {
			background-color: var(--clr-commit-upstream);
		}
		&.remote {
			background-color: var(--clr-commit-remote);
		}
		&.integrated {
			background: repeating-linear-gradient(
				0,
				transparent,
				transparent 0.1875rem,
				var(--clr-commit-shadow) 0.1875rem,
				var(--clr-commit-shadow) 0.4375rem
			);
		}
	}

	.root {
		position: absolute;
		width: 10px;
		top: 30px;
		border-radius: var(--radius-l) 0 0 0;
		height: 20px;
		left: 11px;
		border-color: var(--clr-commit-local);
		border-width: 2px 0 0 2px;
		&.base {
			top: 0px;
		}
	}

	.base-icon {
		display: flex;
		justify-content: center;
		align-items: center;
		position: absolute;
		border-radius: 6px;
		top: var(--base-icon-top);
		left: 50%;
		transform: translateX(-50%);
		background: var(--clr-commit-remote);
		height: 18px;
		width: 18px;
		transition: top var(--transition-medium);

		& svg {
			height: 16px;
			width: 16px;
		}
	}
</style>
