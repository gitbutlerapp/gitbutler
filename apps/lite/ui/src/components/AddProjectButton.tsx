import type { FC } from "react";
import { getButtonClassName, type ButtonSize } from "#ui/components/Button.tsx";

type Props = {
	size?: ButtonSize;
	isPending: boolean;
	onClick: () => void;
};

export const AddProjectButton: FC<Props> = ({ size, isPending, onClick }) => (
	<button
		type="button"
		className={getButtonClassName({ size })}
		disabled={isPending}
		onClick={onClick}
	>
		{isPending ? "Adding repository…" : "Add local repository"}
	</button>
);
