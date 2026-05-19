<script lang="ts" module>
	export interface Props {
		disabled?: boolean;
		loading?: boolean;
		holdMs?: number;
		size?: "tag" | "button";
		style?: ComponentColorType;
		kind?: ComponentKindType;
		wide?: boolean;
		icon?: IconName;
		tooltip?: string;
		testId?: string;
		onconfirm?: () => Promise<void> | void;
		children?: Snippet;
	}
</script>

<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import Tooltip from "$components/Tooltip.svelte";
	import { type IconName } from "$lib/icons/names";
	import type { ComponentColorType, ComponentKindType } from "$lib/utils/colorTypes";
	import type { Snippet } from "svelte";

	let {
		disabled = false,
		loading = false,
		holdMs = 1400,
		size = "button",
		style = "danger",
		kind = "solid",
		wide = false,
		icon,
		tooltip,
		testId,
		onconfirm,
		children,
	}: Props = $props();

	let progress = $state(0);
	let holding = $state(false);
	let confirming = $state(false);
	let startTime = 0;
	let frame: number | undefined;
	let buttonElement = $state<HTMLButtonElement>();

	const isDisabled = $derived(disabled || loading || confirming);
	const iconSize = $derived(size === "tag" ? 14 : 16);
	const buttonClasses = $derived([
		"hold-confirm",
		style,
		kind,
		`${size}-size`,
		wide && "wide",
		holding && "holding",
	]);

	function cancelFrame() {
		if (frame !== undefined) {
			cancelAnimationFrame(frame);
			frame = undefined;
		}
	}

	function reset() {
		cancelFrame();
		holding = false;
		progress = 0;
	}

	function tick() {
		const elapsed = performance.now() - startTime;
		progress = Math.min(elapsed / holdMs, 1);

		if (progress >= 1) {
			confirm();
			return;
		}

		frame = requestAnimationFrame(tick);
	}

	async function confirm() {
		cancelFrame();
		holding = false;
		confirming = true;

		try {
			await onconfirm?.();
		} finally {
			confirming = false;
			progress = 0;
		}
	}

	function start(e: PointerEvent | KeyboardEvent) {
		if (isDisabled || holding) return;
		e.preventDefault();
		startTime = performance.now();
		holding = true;
		progress = 0;
		frame = requestAnimationFrame(tick);

		if (e instanceof PointerEvent) {
			buttonElement?.setPointerCapture(e.pointerId);
		}
	}

	function stop(e?: PointerEvent | KeyboardEvent) {
		e?.preventDefault();
		if (!holding) return;
		reset();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === " " || e.key === "Enter") {
			start(e);
		}
	}

	function handleKeyup(e: KeyboardEvent) {
		if (e.key === " " || e.key === "Enter") {
			stop(e);
		}
	}
</script>

<Tooltip text={tooltip}>
	<button
		bind:this={buttonElement}
		class={buttonClasses}
		style:--hold-progress={`${progress * 100}%`}
		disabled={isDisabled}
		onpointerdown={start}
		onpointerup={stop}
		onpointercancel={stop}
		onpointerleave={stop}
		onkeydown={handleKeydown}
		onkeyup={handleKeyup}
		{...testId ? { "data-testid": testId } : {}}
	>
		<span class="hold-fill"></span>
		<span class="hold-content">
			{#if loading || confirming}
				<Icon name="spinner" size={iconSize} />
			{:else if icon}
				<Icon name={icon} size={iconSize} />
			{/if}
			<span class="hold-label text-semibold truncate text-12">
				{#if children}
					{@render children()}
				{/if}
			</span>
		</span>
	</button>
</Tooltip>

<style lang="postcss">
	:where(.hold-confirm) {
		display: inline-flex;
		position: relative;
		align-items: center;
		justify-content: center;
		overflow: hidden;
		border-radius: var(--radius-button);
		background: var(--btn-bg);
		color: var(--label-clr);
		cursor: pointer;
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast);
		user-select: none;

		&.tag-size {
			--btn-size: var(--size-tag);
			--btn-padding: 2px 6px;
			--btn-gap: 4px;
		}

		&.button-size {
			--btn-size: var(--size-button);
			--btn-padding: 4px 8px;
			--btn-gap: 6px;
		}

		&[class*="-size"] {
			height: var(--btn-size);
			padding: var(--btn-padding);
		}

		&.wide {
			display: flex;
			width: 100%;
		}

		:where(&.danger) {
			--_solid-bg: var(--fill-danger-bg);
			--_solid-fg: var(--fill-danger-fg);
			--_outline-text: var(--btn-danger-outline-text);
			--_outline-bg: var(--btn-danger-outline-bg);
			--_outline-border: var(--btn-danger-outline);
			--_focus-ring: var(--fill-danger-bg);
		}

		:where(&.warning) {
			--_solid-bg: var(--fill-warn-bg);
			--_solid-fg: var(--fill-warn-fg);
			--_outline-text: var(--btn-warn-outline-text);
			--_outline-bg: var(--btn-warn-outline-bg);
			--_outline-border: var(--btn-warn-outline);
			--_focus-ring: var(--fill-warn-bg);
		}

		:where(&.pop) {
			--_solid-bg: var(--fill-pop-bg);
			--_solid-fg: var(--fill-pop-fg);
			--_outline-text: var(--btn-pop-outline-text);
			--_outline-bg: var(--btn-pop-outline-bg);
			--_outline-border: var(--btn-pop-outline);
			--_focus-ring: var(--fill-pop-bg);
		}

		:where(&.solid) {
			--label-clr: var(--_solid-fg);
			--btn-bg: var(--_solid-bg);
			border: 1px solid transparent;
		}

		:where(&.outline) {
			--label-clr: var(--_outline-text);
			--btn-bg: color-mix(in srgb, var(--_outline-bg) 10%, transparent);
			border: 1px solid
				color-mix(in srgb, var(--_outline-border) var(--btn-opacity-outline-border), transparent);
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}

		&:focus-visible {
			outline: 2px solid var(--_focus-ring);
			outline-offset: -2px;
		}
	}

	.hold-fill {
		position: absolute;
		inset: 0 auto 0 0;
		width: var(--hold-progress);
		background: color-mix(in srgb, currentColor 24%, transparent);
		pointer-events: none;
	}

	.hold-content {
		display: inline-flex;
		z-index: 1;
		align-items: center;
		gap: var(--btn-gap);
		min-width: 0;
		pointer-events: none;
	}

	.hold-label {
		overflow: hidden;
		white-space: nowrap;
	}
</style>
