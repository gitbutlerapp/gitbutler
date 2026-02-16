import CardGroupItem from '$components/cardGroup/CardGroupItem.svelte';
import CardGroupRoot from '$components/cardGroup/CardGroupRoot.svelte';

type CardGroupType = typeof CardGroupRoot & {
	Item: typeof CardGroupItem;
};

const CardGroup = Object.assign(CardGroupRoot, {
	Item: CardGroupItem
}) as CardGroupType;

export { CardGroup };
export { CardGroupItem };
