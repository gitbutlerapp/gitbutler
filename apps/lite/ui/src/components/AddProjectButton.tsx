import type { FC } from "react";
import { Button } from "@gitbutler/ui-react/Button.tsx";

type Props = {
	isPending: boolean;
	onClick: () => void;
};

export const AddProjectButton: FC<Props> = ({ isPending, onClick }) => (
	<Button disabled={isPending} onClick={onClick}>
		{isPending ? "Adding repository…" : "Add local repository"}
	</Button>
);
