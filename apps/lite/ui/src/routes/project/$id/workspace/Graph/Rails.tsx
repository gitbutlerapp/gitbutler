import type { VirtualItem } from "@tanstack/react-virtual";
import type { FC } from "react";
import styles from "./Rails.module.css";
import { CARD_GAP, type Card, rails } from "./layout.ts";

/*
 * One SVG behind the cards, drawing the rails through the gaps between them
 * from the virtualiser's measurements, so the gaps beside cards scrolled out
 * of the DOM get theirs too. Every card draws its own rails to its edges,
 * and the upstream section draws its own from its first row, in
 * Section.module.css.
 */
export const Rails: FC<{
	/**
	 * The cards' measurements in plan order: the virtualiser's own cache,
	 * replaced as cards are measured. Read after its total, which refreshes
	 * it, and by index: it is a lazy view whose items exist once read, so
	 * anything that skips holes, `map` included, misses the unread ones.
	 */
	cards: ArrayLike<VirtualItem>;
	/** Where the cards end; null without a base, when there is nothing to fork off and nothing is drawn. */
	cardsEnd: number | null;
}> = ({ cards, cardsEnd }) => {
	const measured: Array<Card> = [];
	for (let index = 0; index < cards.length; index++) {
		const item = cards[index];
		if (item !== undefined) measured.push({ topY: item.start, bottomY: item.end });
	}
	const paths = cardsEnd === null ? [] : rails(measured, cardsEnd);
	return (
		<svg
			className={styles.rails}
			width="100%"
			height={cardsEnd === null ? 0 : cardsEnd + CARD_GAP}
			aria-hidden
		>
			{paths.map((d, index) => (
				// oxlint-disable-next-line react/no-array-index-key -- Every measure replaces the whole list.
				<path key={index} className={styles.lane} d={d} />
			))}
		</svg>
	);
};
