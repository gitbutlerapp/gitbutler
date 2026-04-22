import { moveOperation, rubOperation, type Operation } from "#ui/Operation.ts";
import { Match } from "effect";
import { itemEquals, type Item } from "./Item.ts";
import { type OperationMode } from "./WorkspaceMode.ts";

export const operationModeToOperation = ({
	operationMode,
	source,
	target,
}: {
	operationMode: OperationMode;
	source: Item;
	target: Item;
}): Operation | null =>
	Match.value(operationMode).pipe(
		Match.tagsExhaustive({
			Rub: () => rubOperation({ source, target }),
			Move: () => moveOperation({ source, target, side: "below" }),
		}),
	);

export const isOperationModeSourceOrTarget = ({
	item,
	operationMode,
	source,
}: {
	item: Item;
	operationMode: OperationMode;
	source: Item;
}): boolean =>
	itemEquals(operationMode.source, item) ||
	!!operationModeToOperation({
		operationMode,
		source,
		target: item,
	});
