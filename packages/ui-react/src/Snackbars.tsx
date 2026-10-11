import { Toast, type ToastManager, type ToastManagerEvent } from "@base-ui/react";
import { type FC, useEffect, useEffectEvent, useState } from "react";
import { classes } from "./classes.ts";
import type { IconName } from "./iconNames.ts";
import { Snackbar, type SnackbarVariant } from "./Snackbar.tsx";
import styles from "./Snackbars.module.css";

/**
 * What a snackbar says about how the news landed, passed to `add({ type })`: a Snackbar variant, or
 * `loading` for work still running, which shows a spinner, has no timer, and is updated to its
 * result once the work is done.
 *
 * @public
 */
export type SnackbarType = SnackbarVariant | "loading";

/**
 * What a snackbar carries in Base UI's `data`: `add({ data: { icon, dismissable } })`.
 *
 * @public
 */
export type SnackbarData = {
	/** Overrides the variant's own glyph. */
	icon?: IconName;
	/** Gives it a close button. Pair it with `timeout: 0` for one that waits to be dealt with. */
	dismissable?: boolean;
};

const variant = (type: string | undefined): SnackbarVariant =>
	type === "warning" || type === "danger" || type === "safe" ? type : "info";

/** Where a snackbar's own `timeout` goes once it is taken off Base UI. */
type Timed = { lifetime?: number };

// Base UI pauses every timer while the window is out of focus, so a snackbar raised just before
// switching away is still there, frozen, on the way back: news that should have gone keeps
// waiting. The stack hands Base UI each snackbar with no timer and keeps the time itself.
const untimed = (event: ToastManagerEvent): ToastManagerEvent => {
	// Base UI types the event's options as `any`; an add or an update carries a toast's fields.
	const options = event.options as { timeout?: number };
	if ((event.action !== "add" && event.action !== "update") || !Object.hasOwn(options, "timeout"))
		return event;
	const { timeout: lifetime, ...rest } = options;
	return { action: event.action, options: { ...rest, lifetime } };
};

const relays = new WeakMap<ToastManager<SnackbarData>, ToastManager<SnackbarData>>();

const relay = (manager: ToastManager<SnackbarData>): ToastManager<SnackbarData> => {
	let relayed = relays.get(manager);
	if (relayed === undefined) {
		relayed = {
			...manager,
			" subscribe": (listener) => manager[" subscribe"]((event) => listener(untimed(event))),
		};
		relays.set(manager, relayed);
	}
	return relayed;
};

/** Lets a snackbar go once `duration` has run without a pause; each pause starts it over. */
const Countdown: FC<{ duration: number; paused: boolean; restart: unknown; onEnd: () => void }> = ({
	duration,
	paused,
	restart,
	onEnd,
}) => {
	const end = useEffectEvent(onEnd);
	useEffect(() => {
		if (paused || duration <= 0) return;
		const timer = setTimeout(end, duration);
		return () => clearTimeout(timer);
	}, [duration, paused, restart]);
	return null;
};

const Stack: FC<{ timeout: number; className?: string }> = ({ timeout, className }) => {
	const { toasts, close } = Toast.useToastManager<SnackbarData>();
	const [hovered, setHovered] = useState(false);
	// Base UI paused on focus inside the stack too, which its timers no longer see: a snackbar
	// reached with the keyboard stays while its button is focused.
	const [focused, setFocused] = useState(false);
	return (
		<Toast.Portal>
			<Toast.Viewport
				className={classes(styles.viewport, className)}
				onMouseEnter={() => setHovered(true)}
				onMouseLeave={() => setHovered(false)}
				onFocus={() => setFocused(true)}
				onBlur={(event) => {
					if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
				}}
			>
				{toasts.map((snackbar, index) => (
					<Toast.Root
						key={snackbar.id}
						toast={snackbar}
						className={styles.root}
						// A leaving snackbar keeps its place in the list but not its visible index, so this is
						// how the stylesheet tells the front one, which sinks out, from the rest.
						data-front={index === 0 || undefined}
						// On the root rather than the snackbar: the root captures the pointer for swiping, so a
						// click lands here.
						onClick={() => close(snackbar.id)}
					>
						<Countdown
							duration={snackbar.type === "loading" ? 0 : ((snackbar as Timed).lifetime ?? timeout)}
							paused={hovered || focused}
							// A busy one turned into its result, or news raised again under the same id, gets
							// the whole time again.
							restart={snackbar.updateKey}
							onEnd={() => close(snackbar.id)}
						/>
						<Snackbar
							// The root announces it, through the title; the snackbar itself stays quiet.
							role="none"
							variant={variant(snackbar.type)}
							icon={snackbar.type === "loading" ? "spinner" : snackbar.data?.icon}
							onDismiss={snackbar.data?.dismissable ? () => close(snackbar.id) : undefined}
						>
							<Toast.Title render={<span />} />
						</Snackbar>
					</Toast.Root>
				))}
			</Toast.Viewport>
		</Toast.Portal>
	);
};

/**
 * Where snackbars appear: a stack at the bottom centre of the window. Render it once with a manager
 * of its own, separate from the toasts', from Base UI's `Toast.createToastManager<SnackbarData>()`.
 * Raise one with `manager.add({ title, type })`; `update(id, …)` turns a `loading` one into its
 * result, and `close(id)` lets it go.
 *
 * They stack one below another, 8px apart, the newest at the bottom. Each rises in as it arrives and
 * sinks out as it goes while the rest close the gap, and a click anywhere on one lets it go early.
 * One on a timer leaves after `timeout`, or its own `add({ timeout })`, and starts over while the
 * pointer or keyboard focus is on the stack. Unlike toasts, it keeps going while the window is out
 * of focus.
 *
 * @public
 * @import import { Snackbars } from "@gitbutler/ui-react/Snackbars.tsx";
 */
export const Snackbars: FC<{
	manager: ToastManager<SnackbarData>;
	/** How long one on a timer stays, in milliseconds. */
	timeout?: number;
	/** On the stack, for a host that lifts it with `--snackbars-bottom`. */
	className?: string;
}> = ({ manager, timeout = 5000, className }) => (
	<Toast.Provider toastManager={relay(manager)} timeout={0}>
		<Stack timeout={timeout} className={className} />
	</Toast.Provider>
);
