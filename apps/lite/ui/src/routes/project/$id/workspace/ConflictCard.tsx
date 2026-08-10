import { getButtonClassName } from "#ui/components/Button.tsx";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { classes } from "#ui/components/classes.ts";
import { FieldTextareaStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import {
	attachResolvedLanguages,
	getFiletypeFromFileName,
	getHighlighterIfLoaded,
	getResolvedOrResolveLanguage,
	type SupportedLanguages,
} from "@pierre/diffs";
import { useEffect, useMemo, useState, type FC } from "react";
import styles from "./ConflictCard.module.css";
import type { ConflictHunk, HunkResolution } from "@gitbutler/but-sdk";

type Props = {
	projectId: string;
	/** The conflicted commit, which the check is scoped to. */
	commitId: string;
	path: string;
	/** 1-based, as the resolve API addresses conflicts. */
	hunk: number;
	conflict: ConflictHunk;
	/** How many conflicts the file has, for the "N of M" title. */
	total: number;
	/** The shiki theme the diff renders with, so the sides match the code around them. */
	codeTheme: string;
	/**
	 * Whether the diff around this card is showing two columns. The sides are
	 * named for where they are on screen, and only a split has a left and right.
	 */
	splitView: boolean;
	/**
	 * True while any card's resolution is in flight. The mutation is owned one
	 * level up so this covers every card: each apply rewrites the commit, so a
	 * second one started meanwhile would address an id that no longer exists.
	 */
	busy: boolean;
	onResolve: (path: string, hunk: number, resolution: HunkResolution) => void;
};

/**
 * A conflict side as highlighted code.
 *
 * Uses the highlighter the diff already loaded — a card only ever renders
 * inside a rendered diff, so it is there — and falls back to plain text when a
 * language or theme has not been resolved, which throws rather than degrading.
 */
const Code: FC<{ code: string; path: string; theme: string }> = (p) => {
	// Bumped once a language had to be fetched, to recompute with it attached.
	const [attachedLanguages, setAttachedLanguages] = useState(0);
	const lang = getFiletypeFromFileName(p.path);

	const tokens = useMemo(() => {
		// Read so the dependency is a real one: it is the retry signal, marking
		// the attempt below stale once a language has been attached.
		void attachedLanguages;
		const highlighter = getHighlighterIfLoaded();
		if (!highlighter) return null;
		try {
			return highlighter.codeToTokens(p.code, { lang, theme: p.theme });
		} catch {
			// The language is not attached yet; the effect below fetches it.
			return null;
		}
	}, [p.code, p.theme, lang, attachedLanguages]);

	useEffect(() => {
		const highlighter = getHighlighterIfLoaded();
		if (tokens !== null || !highlighter || lang === "text" || lang === "ansi") return;
		// The diff attaches languages as it needs them, and a card can render
		// before this file's is there. Resolving and attaching is what the
		// renderer itself does, so it adds to the shared highlighter rather than
		// displacing what the diff already has.
		let stale = false;
		Promise.resolve(
			getResolvedOrResolveLanguage(lang as Exclude<SupportedLanguages, "text" | "ansi">),
		)
			.then((resolved) => {
				if (stale) return;
				attachResolvedLanguages(resolved, highlighter);
				setAttachedLanguages((n) => n + 1);
			})
			.catch(() => {});
		return () => {
			stale = true;
		};
	}, [tokens, lang]);

	return (
		<pre className={classes(styles.content, "text-12")}>
			{tokens === null
				? p.code
				: tokens.tokens.map((line, lineIndex) => (
						// Lines and tokens have no identity beyond their position.
						// oxlint-disable-next-line no-array-index-key
						<span key={lineIndex}>
							{line.map((token, tokenIndex) => (
								// oxlint-disable-next-line no-array-index-key
								<span key={tokenIndex} style={{ color: token.color }}>
									{token.content}
								</span>
							))}
							{"\n"}
						</span>
					))}
		</pre>
	);
};

/** One unresolved conflict, rendered inline in the diff where its region starts. */
export const ConflictCard: FC<Props> = (p) => {
	const dispatch = useAppDispatch();
	const [draft, setDraft] = useState<string | null>(null);
	const conflict = { commitId: p.commitId, path: p.path, hunk: p.hunk };
	// A primitive, so checking one conflict re-renders one card rather than all.
	const checked = useAppSelector((state) =>
		projectSlice.selectors.selectIsConflictChecked(state, p.projectId, conflict),
	);
	// Checked conflicts are resolved from the bar, so their own resolutions go
	// inert — disabled rather than hidden, which would change the card's height
	// and shift every conflict below it as you work down the list. The checkbox
	// and Cancel stay live, or checking a card would trap you in it.
	const disabled = p.busy || checked;

	const apply = (resolution: HunkResolution) => {
		if (disabled) return;
		p.onResolve(p.path, p.hunk, resolution);
	};

	return (
		<div className={styles.wrapper}>
			<header className={styles.header}>
				<Checkbox
					checked={checked}
					disabled={p.busy}
					aria-label={`Check conflict ${p.hunk} in ${p.path}`}
					onCheckedChange={(next) =>
						dispatch(
							projectSlice.actions.checkConflict({
								projectId: p.projectId,
								conflict,
								checked: next,
							}),
						)
					}
				/>
				<Icon name="warning" />
				<h5 className={classes(styles.title, "text-13")}>
					{p.total > 1 ? `Unresolved conflict ${p.hunk} of ${p.total}` : "Unresolved conflict"}
				</h5>
			</header>

			{/* Only the ancestor. The diff either side of this card is already the
			    base's version against the commit's own, so repeating those two
			    would say twice what is on screen once. */}
			<div className={styles.side}>
				<div className={classes(styles.label, "text-12")}>Common ancestor</div>
				{p.conflict.base === null ? (
					<div className={classes(styles.hint, "text-12")}>
						The merge found no common ancestor for this region.
					</div>
				) : (
					<Code code={p.conflict.base} path={p.path} theme={p.codeTheme} />
				)}
			</div>

			{draft === null ? (
				<div className={styles.actions}>
					<button
						type="button"
						className={getButtonClassName({ variant: "outline", size: "small" })}
						disabled={disabled}
						onClick={() => apply({ type: "ours" })}
					>
						{p.splitView ? "Use left" : "Use removed"}
					</button>
					<button
						type="button"
						className={getButtonClassName({ variant: "outline", size: "small" })}
						disabled={disabled}
						onClick={() => apply({ type: "theirs" })}
					>
						{p.splitView ? "Use right" : "Use added"}
					</button>
					<button
						type="button"
						className={getButtonClassName({ variant: "ghost", size: "small" })}
						disabled={disabled}
						onClick={() => setDraft(p.conflict.theirs)}
					>
						Edit
					</button>
				</div>
			) : (
				<>
					<div className={classes(styles.hint, "text-12")}>
						This replaces the whole conflicted region. Leaving it empty deletes the region; never
						include conflict markers.
					</div>
					<FieldTextareaStyles
						className={classes(styles.content, "text-12")}
						value={draft}
						rows={Math.min(Math.max(draft.split("\n").length + 1, 3), 16)}
						spellCheck={false}
						disabled={disabled}
						onChange={(event) => setDraft(event.currentTarget.value)}
					/>
					<div className={styles.actions}>
						<button
							type="button"
							className={getButtonClassName({ variant: "pop", size: "small" })}
							disabled={disabled}
							onClick={() => apply({ type: "content", subject: draft })}
						>
							Apply resolution
						</button>
						<button
							type="button"
							className={getButtonClassName({ variant: "ghost", size: "small" })}
							disabled={p.busy}
							onClick={() => setDraft(null)}
						>
							Cancel
						</button>
					</div>
				</>
			)}
		</div>
	);
};
