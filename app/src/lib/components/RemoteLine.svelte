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
		<div class="remote-line dashed short" />
		{#if outType}
			<div
				class="remote-line base tip"
				class:dashed={outDashed}
				class:upstream={outType == 'upstream'}
			/>
		{/if}
		{#if root}
			<div class="root base" />
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
				class:upstream={outType == 'upstream'}
				class:remote={outType == 'remote'}
				class:dashed={outDashed}
				class:first={sectionFirst}
			/>
		{/if}
		{#if inType}
			<div
				class="remote-line short"
				class:integrated
				class:first={sectionFirst}
				class:upstream={inType == 'upstream'}
				class:remote={inType == 'remote'}
				class:dashed={inDashed}
			/>
		{/if}
		{#if root}
			<div class="root" />
		{/if}
		<slot />
	{/if}
</div>

<style lang="postcss">
	.remote-column {
		position: relative;
		width: var(--size-24);
	}

	.remote-line {
		position: absolute;
		width: var(--size-2);
		background-color: var(--clr-commit-remote);
		left: calc(var(--size-10) + 0.063rem);
		bottom: 0;
		top: 0;
		&.short {
			top: calc(var(--avatar-top) + var(--size-4));
			&.first {
				top: calc(var(--avatar-first-top) + var(--size-4));
			}
			&.base {
				top: calc(var(--avatar-top) + var(--size-8));
			}
		}
		&.tip {
			bottom: calc(100% - var(--avatar-top) - var(--size-4));
			&.first {
				bottom: calc(100% - var(--avatar-first-top) - var(--size-4));
			}
			&.base {
				bottom: calc(100% - 1.5rem);
			}
		}
		&.dashed {
			background: repeating-linear-gradient(
				0,
				transparent,
				transparent 0.1875rem,
				var(--clr-commit-remote) 0.1875rem,
				var(--clr-commit-remote) 0.4375rem
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
		width: var(--size-10);
		top: 1.875rem;
		border-radius: var(--radius-l) 0 0 0;
		height: var(--size-20);
		left: calc(var(--size-10) + 0.063rem);
		border-color: var(--clr-commit-local);
		border-width: var(--size-2) 0 0 var(--size-2);
		&.base {
			top: -1px;
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
		height: 1.125rem;
		width: 1.125rem;
		transition: top var(--transition-medium);

		& svg {
			height: var(--size-16);
			width: var(--size-16);
		}
	}
</style>
