declare type Item = import('svelte-dnd-action').Item;
declare type DndEvent<ItemType = Item> = import('svelte-dnd-action').DndEvent<ItemType>;
declare namespace svelte.JSX {
	interface HTMLAttributes<T> {
		onconsider?: (event: CustomEvent<DndEvent<ItemType>> & { target: EventTarget & T }) => void;
		onfinalize?: (event: CustomEvent<DndEvent<ItemType>> & { target: EventTarget & T }) => void;
	}
}
