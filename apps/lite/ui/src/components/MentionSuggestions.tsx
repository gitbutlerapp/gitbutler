import { reviewerCandidatesQueryOptions } from "#ui/api/queries.ts";
import { Popup, PopupItem } from "#ui/components/Popup.tsx";
import { applyToTextarea } from "#ui/markdown-textarea.ts";
import { completeMention, matchMentions, mentionAtCaret } from "#ui/mentions.ts";
import { Popover } from "@base-ui/react";
import type { ForgeReviewUser } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import {
	type ChangeEvent,
	type KeyboardEvent,
	type RefObject,
	type SyntheticEvent,
	useId,
	useMemo,
	useState,
} from "react";
import styles from "./MentionSuggestions.module.css";

type Props = {
	projectId: string;
	targetRef: RefObject<HTMLTextAreaElement | null>;
	value: string;
	onInput: (value: string) => void;
};

/** The textarea styles that decide where its text wraps. */
const LAYOUT_STYLES = [
	"box-sizing",
	"width",
	"padding",
	"border-width",
	"font-family",
	"font-size",
	"font-weight",
	"font-style",
	"line-height",
	"letter-spacing",
	"tab-size",
];

/**
 * Where the character at `index` sits on screen. A textarea does not expose
 * the layout of its text, so a hidden copy is laid out the same way with that
 * character wrapped in a span, and the span is measured instead.
 */
const characterRect = (textarea: HTMLTextAreaElement, index: number): DOMRect => {
	const mirror = document.createElement("div");
	const computed = getComputedStyle(textarea);
	for (const property of LAYOUT_STYLES)
		mirror.style.setProperty(property, computed.getPropertyValue(property));
	mirror.style.position = "absolute";
	mirror.style.visibility = "hidden";
	mirror.style.whiteSpace = "pre-wrap";
	mirror.style.overflowWrap = "break-word";
	mirror.textContent = textarea.value.slice(0, index);
	const marker = document.createElement("span");
	const char = textarea.value.charAt(index);
	marker.textContent = char === "" ? " " : char;
	// Full line height, so the popup hangs under the line rather than the glyph.
	marker.style.display = "inline-block";
	mirror.append(marker);
	document.body.append(mirror);

	const host = textarea.getBoundingClientRect();
	const rect = new DOMRect(
		host.left + textarea.clientLeft + marker.offsetLeft - textarea.scrollLeft,
		host.top + textarea.clientTop + marker.offsetTop - textarea.scrollTop,
		marker.offsetWidth,
		marker.offsetHeight,
	);
	mirror.remove();
	return rect;
};

/**
 * Offer collaborators as an `@` is typed and complete the one chosen. The
 * returned props go on the textarea, which keeps focus throughout; `onKeyDown`
 * returns whether the popup took the key, so the owner can handle the rest.
 */
export const useMentionSuggestions = ({ projectId, targetRef, value, onInput }: Props) => {
	// Tracked from onChange as well as onSelect: onSelect fires a task later,
	// and a render in between would pair the new text with the old caret.
	const [selectionStart, setSelectionStart] = useState(0);
	const [selectionEnd, setSelectionEnd] = useState(0);
	// Escape closes the popup for this `@` until the caret backs out past it.
	const [dismissedAt, setDismissedAt] = useState<number | null>(null);
	const [highlighted, setHighlighted] = useState(0);
	const listId = useId();

	// By hand: the compiler cannot tell `mentionAtCaret` is pure, and without
	// this it rebuilds every closure below on each render, the query's `select` too.
	const mention = useMemo(
		() => mentionAtCaret({ text: value, start: selectionStart, end: selectionEnd }),
		[value, selectionStart, selectionEnd],
	);
	const at = mention?.at ?? null;
	const query = mention?.query ?? null;
	const { data: matches = [] } = useQuery({
		...reviewerCandidatesQueryOptions(projectId),
		select: (candidates) => (query === null ? [] : matchMentions(candidates, query)),
	});
	const open = at !== null && at !== dismissedAt && matches.length > 0;
	const highlightedUser = matches[highlighted];
	const optionId = (user: ForgeReviewUser) => `${listId}-${user.id}`;

	const track = (target: HTMLTextAreaElement) => {
		setSelectionStart(target.selectionStart);
		setSelectionEnd(target.selectionEnd);
		setHighlighted(0);
		if (dismissedAt !== null && target.selectionEnd <= dismissedAt) setDismissedAt(null);
	};

	const accept = (user: ForgeReviewUser) => {
		const target = targetRef.current;
		if (target === null || at === null) return;
		onInput(applyToTextarea(target, completeMention(at, user.login)));
		track(target);
	};

	const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>): boolean => {
		if (at === null || !open || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey)
			return false;
		switch (event.key) {
			case "ArrowDown":
				setHighlighted((highlighted + 1) % matches.length);
				break;
			case "ArrowUp":
				setHighlighted((highlighted + matches.length - 1) % matches.length);
				break;
			case "Enter":
			case "Tab":
				if (highlightedUser === undefined) return false;
				accept(highlightedUser);
				break;
			case "Escape":
				setDismissedAt(at);
				break;
			default:
				return false;
		}
		event.preventDefault();
		// Keeps the window's own Escape and hotkeys out of it.
		event.stopPropagation();
		return true;
	};

	// Anchored on the `@` rather than the caret, so the popup holds still as the query grows.
	const anchor = () => {
		const target = targetRef.current;
		return target === null || at === null
			? null
			: { contextElement: target, getBoundingClientRect: () => characterRect(target, at) };
	};

	const popup = (
		<Popover.Root
			open={open}
			onOpenChange={(next) => {
				if (!next && at !== null) setDismissedAt(at);
			}}
		>
			{/* Unmounted rather than closed: a closing animation would play over an emptied list. */}
			{open && (
				<Popover.Portal>
					<Popover.Positioner anchor={anchor} side="bottom" align="start" sideOffset={4}>
						<Popup
							anchored
							className={styles.popup}
							render={<Popover.Popup initialFocus={false} finalFocus={false} />}
						>
							{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- No native list floats under a textarea. */}
							<div aria-label="Mention someone" className={styles.list} id={listId} role="listbox">
								{matches.map((user, index) => (
									<PopupItem
										aria-selected={index === highlighted}
										data-highlighted={index === highlighted || undefined}
										id={optionId(user)}
										key={user.id}
										onClick={() => accept(user)}
										// Keeps focus, and so the caret, in the textarea.
										onMouseDown={(event) => event.preventDefault()}
										onPointerMove={() => setHighlighted(index)}
										// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Same as the listbox.
										role="option"
										tabIndex={-1}
									>
										<span className={styles.row}>
											{user.avatarUrl !== null ? (
												<img src={user.avatarUrl} className={styles.avatar} alt="" />
											) : (
												<span className={styles.avatar} />
											)}
											<span className={styles.login}>{user.login}</span>
											{user.name !== null && <span className={styles.name}>{user.name}</span>}
										</span>
									</PopupItem>
								))}
							</div>
						</Popup>
					</Popover.Positioner>
				</Popover.Portal>
			)}
		</Popover.Root>
	);

	return {
		textareaProps: {
			onChange: (event: ChangeEvent<HTMLTextAreaElement>) => {
				onInput(event.currentTarget.value);
				track(event.currentTarget);
			},
			onSelect: (event: SyntheticEvent<HTMLTextAreaElement>) => track(event.currentTarget),
			"aria-autocomplete": "list" as const,
			"aria-controls": open ? listId : undefined,
			"aria-activedescendant":
				open && highlightedUser !== undefined ? optionId(highlightedUser) : undefined,
		},
		onKeyDown,
		popup,
	};
};
