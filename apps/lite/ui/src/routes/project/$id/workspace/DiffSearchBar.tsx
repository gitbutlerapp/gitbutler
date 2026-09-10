import { assert } from "#ui/assert.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { FieldControlStyles, FieldRootStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Popup } from "#ui/components/Popup.tsx";
import { focusScope } from "#ui/focus-scopes.ts";
import { diffHotkeys } from "#ui/hotkeys.ts";
import { Field } from "@base-ui/react";
import type { CodeViewDiffItem, CodeViewItem } from "@pierre/diffs";
import { useHotkey } from "@tanstack/react-hotkeys";
import {
	type FC,
	type KeyboardEvent,
	type RefObject,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { diffSearchMatches, type DiffSearchMatch, type DiffSearchSource } from "./diff-search.ts";
import styles from "./DiffSearchBar.module.css";

type Props = {
	items: Array<CodeViewItem<unknown>>;
	/** The diff's focus scope, where the open hotkey listens. */
	focusScopeRef: RefObject<HTMLDivElement | null>;
	onNavigate: (match: DiffSearchMatch) => void;
	/** Keeps the viewer's match marks in step with what this bar found. */
	onMatchesChange: (matches: Array<DiffSearchMatch>, current: DiffSearchMatch | null) => void;
	getSearchSource: (item: CodeViewDiffItem<unknown>) => DiffSearchSource | undefined;
};

/**
 * Text search over the whole diff. The viewer virtualizes its rendering, so
 * the browser's own find only sees the rendered window; this searches the
 * model instead and navigates by scrolling the viewer, marking each match
 * with the diff's line selection.
 */
export const DiffSearchBar: FC<Props> = ({
	items,
	focusScopeRef,
	onNavigate,
	onMatchesChange,
	getSearchSource,
}) => {
	/** The query, or `null` while the search is closed. */
	const [query, setQuery] = useState<string | null>(null);
	const [index, setIndex] = useState(0);
	const inputRef = useRef<HTMLInputElement>(null);

	const matches = useMemo(
		() => (query === null ? [] : diffSearchMatches(items, query, getSearchSource)),
		[items, query, getSearchSource],
	);
	// Clamped rather than stored: the diff can refresh under a held index.
	const current = matches.length === 0 ? null : Math.min(index, matches.length - 1);

	// The marks live in the viewer's DOM, outside React, so they are synced as
	// an effect: on every new match list, and away again on unmount. Lifting
	// the matches into the parent instead would re-render the whole diff pane
	// on every keystroke.
	// oxlint-disable react-you-might-not-need-an-effect/no-pass-data-to-parent
	useEffect(() => {
		onMatchesChange(matches, current === null ? null : (matches[current] ?? null));
	}, [matches, current, onMatchesChange]);
	useEffect(() => () => onMatchesChange([], null), [onMatchesChange]);
	// oxlint-enable react-you-might-not-need-an-effect/no-pass-data-to-parent

	// Typing lands on the first match as the query narrows. This waits for the
	// memo above rather than scanning again in the change handler: the scan
	// walks every line of every file, and a second one per keystroke is felt on
	// a large diff. Keyed on the query, so a background diff refresh — which
	// gives `matches` a new identity — does not yank the scroll position.
	const lastNavigatedQuery = useRef<string | null>(null);
	// oxlint-disable react-you-might-not-need-an-effect/no-event-handler
	// oxlint-disable react-you-might-not-need-an-effect/no-pass-data-to-parent
	useEffect(() => {
		if (query === null || query === "" || lastNavigatedQuery.current === query) return;

		lastNavigatedQuery.current = query;
		const first = matches[0];
		if (first) onNavigate(first);
	}, [query, matches, onNavigate]);
	// oxlint-enable react-you-might-not-need-an-effect/no-event-handler
	// oxlint-enable react-you-might-not-need-an-effect/no-pass-data-to-parent

	const open = (): void => {
		if (query === null) {
			setQuery("");
			setIndex(0);
			return;
		}

		const input = inputRef.current;
		if (!input) return;
		input.focus();
		// Land the caret after the query, so returning to it extends the search
		// rather than overwriting it.
		input.setSelectionRange(input.value.length, input.value.length);
	};

	const close = (): void => {
		setQuery(null);
		setIndex(0);
		// Hand focus back to the diff the search was walking, rather than
		// dropping it on the body when the input unmounts.
		focusScope("diff");
	};

	const step = (offset: -1 | 1): void => {
		if (current === null) return;

		const next = (current + offset + matches.length) % matches.length;
		setIndex(next);
		onNavigate(assert(matches[next]));
	};

	useHotkey(diffHotkeys.search.hotkey, open, {
		conflictBehavior: "allow",
		meta: diffHotkeys.search.meta,
		target: focusScopeRef,
	});

	if (query === null) return null;

	const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
		// Escape closes the search rather than reaching anything behind it,
		// which has nothing to cancel while the input holds focus.
		if (event.key === "Escape") {
			event.preventDefault();
			event.stopPropagation();
			close();
			return;
		}

		if (event.key === "Enter") {
			event.preventDefault();
			event.stopPropagation();
			step(event.shiftKey ? -1 : 1);
		}
	};

	return (
		<Popup className={styles.searchBar} render={<search />}>
			<Field.Root render={<FieldRootStyles />}>
				<Field.Control
					ref={inputRef}
					render={<FieldControlStyles />}
					className={styles.searchInput}
					aria-label="Search diff"
					placeholder="Search diff"
					value={query}
					onChange={(event) => {
						setQuery(event.currentTarget.value);
						setIndex(0);
					}}
					onKeyDown={handleKeyDown}
					// oxlint-disable-next-line jsx_a11y/no-autofocus
					autoFocus
				/>
			</Field.Root>

			{/* Over the field's end rather than beside it, so the bar is one width
			    whether or not there is a query, and the field reads as the whole
			    bar the way a browser's find bar does. */}
			<div className={styles.controls}>
				{query !== "" && (
					<>
						<span className={classes("text-13", styles.matchCount)} aria-live="polite">
							{current === null ? 0 : current + 1}/{matches.length}
						</span>
						<span className={styles.divider} />
					</>
				)}
				<div className={styles.buttons}>
					<button
						type="button"
						aria-label="Previous match"
						disabled={matches.length === 0}
						className={getButtonClassName({ size: "small", variant: "ghost", iconOnly: true })}
						onClick={() => step(-1)}
					>
						<Icon name="arrow-up" />
					</button>
					<button
						type="button"
						aria-label="Next match"
						disabled={matches.length === 0}
						className={getButtonClassName({ size: "small", variant: "ghost", iconOnly: true })}
						onClick={() => step(1)}
					>
						<Icon name="arrow-down" />
					</button>
					<button
						type="button"
						aria-label="Close search"
						className={getButtonClassName({ size: "small", variant: "ghost", iconOnly: true })}
						onClick={close}
					>
						<Icon name="cross" />
					</button>
				</div>
			</div>
		</Popup>
	);
};
