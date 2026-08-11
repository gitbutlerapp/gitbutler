import { Kbd } from "#ui/components/Kbd.tsx";
import type { SelectionScope } from "#ui/selection-scopes.ts";
import type { ComponentProps, FC } from "react";
import styles from "./SelectionScopeKbd.module.css";

type Props = Pick<ComponentProps<typeof Kbd>, "hotkey"> & {
	scope: SelectionScope;
};

// oxlint-disable-next-line typescript/no-non-null-assertion
const keyClassName = styles.key!;

export const SelectionScopeKbd: FC<Props> = ({ hotkey, scope }) => (
	<>
		{/* Inline style as we can't interpolate a custom property into the selectors. */}
		<style href={`selection-scope-kbd-${scope}`}>{`
			[data-selection-focus-styles="true"]:has(
				[data-selection-scope="${scope}"]:focus-within
			) [data-selection-scope-kbd="${scope}"] .${keyClassName} {
				background-color: var(--fill-gray-bg);
				color: var(--fill-gray-fg);
			}
		`}</style>

		<span data-selection-scope-kbd={scope}>
			<Kbd hotkey={hotkey} className={styles.keys} keyClassName={keyClassName} />
		</span>
	</>
);
