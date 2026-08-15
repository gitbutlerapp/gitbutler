import { getButtonClassName } from "#ui/components/Button.tsx";
import { Component, type ReactNode } from "react";
import styles from "./ErrorBoundary.module.css";

type Props = {
	children: ReactNode;
	/**
	 * Clears the error when any entry changes, so a view that fails on one
	 * selection recovers by moving off it rather than sitting broken until the
	 * window reloads.
	 *
	 * Key these on the very thing being rendered — the child element itself,
	 * not a URL or selection that merely correlates with it. The reset is
	 * single-shot: it happens after the commit, and if the subtree throws again
	 * because it read state the key had already moved past, that key will not
	 * change a second time and the pane stays broken. Where no such key exists,
	 * pass none and leave recovery to Retry.
	 */
	resetKeys?: ReadonlyArray<unknown>;
	onReset?: () => void;
};

type State = {
	error: Error | null;
	/** The keys the current error was raised under, to compare later ones against. */
	resetKeys: ReadonlyArray<unknown>;
};

const asError = (error: unknown): Error =>
	error instanceof Error
		? error
		: new Error(typeof error === "string" ? error : JSON.stringify(error));

const keysChanged = (before: ReadonlyArray<unknown>, after: ReadonlyArray<unknown>): boolean =>
	before.length !== after.length || before.some((key, index) => !Object.is(key, after[index]));

/**
 * Confines a render failure to the subtree it happened in. React boundaries
 * only see errors thrown while rendering below them: a throw in an event
 * handler, a timer, or in the parent that builds this subtree's elements
 * escapes to the toast in `main.tsx` instead.
 */
export class ErrorBoundary extends Component<Props, State> {
	state: State = { error: null, resetKeys: this.props.resetKeys ?? [] };

	static getDerivedStateFromError(error: unknown): Pick<State, "error"> {
		return { error: asError(error) };
	}

	// Derived during render rather than in componentDidUpdate, which would cost a
	// second render pass to clear.
	static getDerivedStateFromProps(props: Props, state: State): State | null {
		const resetKeys = props.resetKeys ?? [];
		if (!keysChanged(state.resetKeys, resetKeys)) return null;
		return { error: null, resetKeys };
	}

	componentDidCatch(error: unknown): void {
		// The fallback shows only the message, and swallowing the rest once cost
		// a session two wrong bisections.
		// oxlint-disable-next-line no-console
		console.error(error);
	}

	handleRetry(): void {
		this.props.onReset?.();
		this.setState({ error: null, resetKeys: this.props.resetKeys ?? [] });
	}

	render(): ReactNode {
		if (!this.state.error) return this.props.children;

		return (
			<div className={styles.error}>
				<h1 className={styles.errorTitle}>Something went wrong.</h1>
				<div className={styles.errorActions}>
					<button
						type="button"
						className={getButtonClassName({})}
						onClick={() => this.handleRetry()}
					>
						Retry
					</button>
				</div>
				<code className={styles.errorMessage}>{this.state.error.message}</code>
			</div>
		);
	}
}
