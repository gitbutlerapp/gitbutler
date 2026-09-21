import type { FC } from "react";
import { getButtonClassName } from "@gitbutler/ui-react/Button.tsx";

type Props = {
	isPending: boolean;
	onClick: () => void;
};

export const AddProjectButton: FC<Props> = ({ isPending, onClick }) => (
	<button type="button" className={getButtonClassName({})} disabled={isPending} onClick={onClick}>
		{isPending ? "Adding repository…" : "Add local repository"}
	</button>
);
