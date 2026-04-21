import {
	moveOperationSourceToOperation,
	rubOperationSourceToOperation,
	type Operation,
} from "#ui/Operation.ts";
import { Match } from "effect";
import { itemEquals, type Item } from "./Item.ts";
import { type OperationMode } from "./WorkspaceMode.ts";

export const operationModeToOperation = ({
	operationMode,
	target,
}: {
	operationMode: OperationMode;
	target: Item;
}): Operation | null =>
	Match.value(operationMode).pipe(
		Match.tagsExhaustive({
			Rub: () => rubOperationSourceToOperation({ source: operationMode.source, target }),
			Move: () =>
				moveOperationSourceToOperation({ source: operationMode.source, target, side: "below" }),
		}),
	);

export const isOperationModeSourceOrTarget = ({
	item,
	operationMode,
}: {
	item: Item;
	operationMode: OperationMode;
}): boolean =>
	itemEquals(operationMode.source, item) ||
	!!operationModeToOperation({
		operationMode,
		target: item,
	});
