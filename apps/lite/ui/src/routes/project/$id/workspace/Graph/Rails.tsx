import type { VirtualItem } from "@tanstack/react-virtual";
import type { FC } from "react";
import styles from "./Rails.module.css";
import { CARD_GAP, type Card, rails } from "./layout.ts";

/*
 * One SVG behind the cards, drawing the gaps between them from the
 * virtualiser's measurements, so gaps beside unmounted cards are drawn too.
 * Cards and the section draw their own rails.
 */
export const Rails: FC<{
	/** The virtualiser's measurement cache. Read after its total, and by index: it is a lazy view, and `map` skips the unread. */
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
